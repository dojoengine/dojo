use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Result};
use dojo_world::contracts::contract_info::ContractInfo;
use slot::account_sdk::account::session::hash::{Policy, ProvedPolicy};
use slot::account_sdk::account::session::merkle::MerkleTree;
use slot::account_sdk::account::session::SessionAccount;
use slot::session::{FullSessionInfo, PolicyMethod};
use starknet::core::types::Felt;
use starknet::core::utils::get_selector_from_name;
use starknet::macros::felt;
use starknet::providers::Provider;
use tracing::trace;
use url::Url;

// Why the Arc? becaues the Controller account implementation over on `account_sdk` crate is
// riddled with `+ Clone` bounds on its Provider generic. So we explicitly specify that the Provider
// impl here is wrapped in an Arc to satisfy the Clone bound. Otherwise, you would get a 'trait
// bound not satisfied' error.
//
// This type comes from account_sdk, which doesn't derive Debug.
#[allow(missing_debug_implementations)]
pub type ControllerSessionAccount<P> = SessionAccount<Arc<P>>;

/// Create a new Catridge Controller account based on session key.
///
/// For now, Controller guarantees that if the provided network is among one of the supported
/// networks, then the Controller account should exist. If it doesn't yet exist, it will
/// automatically be created when a session is created (ie during the session registration stage).
///
/// # Supported networks
///
/// * Starknet mainnet
/// * Starknet sepolia
/// * Slot hosted networks
#[tracing::instrument(name = "create_controller", skip(rpc_url, provider, contracts))]
pub async fn create_controller<P>(
    // Ideally we can get the url from the provider so we dont have to pass an extra url param here
    rpc_url: Url,
    provider: P,
    contracts: &HashMap<String, ContractInfo>,
) -> Result<ControllerSessionAccount<P>>
where
    P: Provider,
    P: Send + Sync,
{
    let chain_id = provider.chain_id().await?;

    trace!(target: "account::controller", "Loading Slot credentials.");
    let credentials = slot::credential::Credentials::load()?;
    let username = credentials.account.id;

    // Right now, the Cartridge Controller API ensures that there's always a Controller associated
    // with an account, but that might change in the future.
    let Some(contract_address) = credentials.account.controllers.first().map(|c| c.address) else {
        bail!("No Controller is associated with this account.");
    };

    let policies = collect_policies(contract_address, contracts)?;

    // Check if the session exists, if not create a new one
    let session_details = match slot::session::get(chain_id)? {
        Some(session) => {
            trace!(target: "account::controller", expires_at = %session.session.expires_at, policies = session.session.policies.len(), "Found existing session.");

            // Check if the policies have changed
            let is_equal = is_equal_to_existing(&policies, &session);

            if is_equal {
                session
            } else {
                trace!(
                    target: "account::controller",
                    new_policies = policies.len(),
                    existing_policies = session.session.policies.len(),
                    "Policies have changed. Creating new session."
                );

                let session = slot::session::create(rpc_url.clone(), &policies).await?;
                slot::session::store(chain_id, &session)?;
                session
            }
        }

        // Create a new session if not found
        None => {
            trace!(target: "account::controller", %username, chain = format!("{chain_id:#}"), "Creating new session.");
            let session = slot::session::create(rpc_url.clone(), &policies).await?;
            slot::session::store(chain_id, &session)?;
            session
        }
    };

    Ok(session_details.into_account(Arc::new(provider)))
}

// Check if the new policies are equal to the ones in the existing session
//
// This function would compute the merkle root of the new policies and compare it with the root in
// the existing SessionMetadata.
fn is_equal_to_existing(new_policies: &[PolicyMethod], session_info: &FullSessionInfo) -> bool {
    let new_policies = new_policies
        .iter()
        .map(|p| Policy::new(p.target, get_selector_from_name(&p.method).unwrap()))
        .collect::<Vec<Policy>>();

    // Copied from Session::new
    let hashes = new_policies.iter().map(Policy::as_merkle_leaf).collect::<Vec<Felt>>();

    let new_policies = new_policies
        .into_iter()
        .enumerate()
        .map(|(i, policy)| ProvedPolicy {
            policy,
            proof: MerkleTree::compute_proof(hashes.clone(), i),
        })
        .collect::<Vec<ProvedPolicy>>();

    let new_policies_root = MerkleTree::compute_root(hashes[0], new_policies[0].proof.clone());
    new_policies_root == session_info.session.authorization_root
}

/// Policies are the building block of a session key. It's what defines what methods are allowed for
/// an external signer to execute using the session key.
///
/// This function collect all the contracts' methods in the current project according to the
/// project's base manifest ( `/manifests/<profile>/base` ) and convert them into policies.
fn collect_policies(
    user_address: Felt,
    contracts: &HashMap<String, ContractInfo>,
) -> Result<Vec<PolicyMethod>> {
    let policies = collect_policies_from_contracts(user_address, contracts)?;
    trace!(target: "account::controller", policies_count = policies.len(), "Extracted policies from project.");
    Ok(policies)
}

fn collect_policies_from_contracts(
    user_address: Felt,
    contracts: &HashMap<String, ContractInfo>,
) -> Result<Vec<PolicyMethod>> {
    let mut policies: Vec<PolicyMethod> = Vec::new();

    for (tag, info) in contracts {
        for e in &info.entrypoints {
            let policy = PolicyMethod { target: info.address, method: e.clone() };
            trace!(target: "account::controller", tag, target = format!("{:#x}", policy.target), method = %policy.method, "Adding policy");
            policies.push(policy);
        }
    }

    // special policy for sending declare tx
    // corresponds to [account_sdk::account::DECLARATION_SELECTOR]
    let method = "__declare_transaction__".to_string();
    policies.push(PolicyMethod { target: user_address, method });
    trace!(target: "account::controller", "Adding declare transaction policy");

    // for deploying using udc
    let method = "deployContract".to_string();
    const UDC_ADDRESS: Felt =
        felt!("0x041a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf");
    policies.push(PolicyMethod { target: UDC_ADDRESS, method });
    trace!(target: "account::controller", "Adding UDC deployment policy");

    Ok(policies)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dojo_test_utils::setup::TestSetup;
    use dojo_world::contracts::ContractInfo;
    use scarb_interop::{MetadataDojoExt, Profile};
    use starknet::macros::felt;

    use super::{collect_policies, PolicyMethod};

    #[test]
    fn collect_policies_from_project() {
        let setup = TestSetup::from_examples("../../crates/dojo/core", "../../examples/");
        let scarb_metadata = setup.load_metadata("spawn-and-move", Profile::DEV);

        let manifest =
            scarb_metadata.read_dojo_manifest_profile().expect("Failed to read manifest").unwrap();
        let contracts: HashMap<String, ContractInfo> = (&manifest).into();

        let user_addr = felt!("0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba");

        let policies = collect_policies(user_addr, &contracts).unwrap();

        if std::env::var("POLICIES_FIX").is_ok() {
            let policies_json = serde_json::to_string_pretty(&policies).unwrap();
            println!("{}", policies_json);
        } else {
            let test_data = include_str!("../../../../tests/test_data/policies.json");
            let expected_policies: Vec<PolicyMethod> = serde_json::from_str(test_data).unwrap();

            // Compare the collected policies with the test data.
            assert_eq!(policies.len(), expected_policies.len());
            expected_policies.iter().for_each(|p| {
                assert!(policies.contains(p), "Policy method '{}' is missing", p.method)
            });
        }
    }
}
