use std::sync::Arc;

use anyhow::{bail, Result};
use dojo_world::diff::WorldDiff;
use dojo_world::ResourceType;
use slot::account_sdk::account::session::hash::{Policy, ProvedPolicy};
use slot::account_sdk::account::session::merkle::MerkleTree;
use slot::account_sdk::account::session::SessionAccount;
use slot::session::{FullSessionInfo, PolicyMethod};
use starknet::core::types::contract::{AbiEntry, StateMutability};
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
#[tracing::instrument(
    name = "create_controller",
    skip(rpc_url, provider, world_address, world_diff)
)]
pub async fn create_controller<P>(
    // Ideally we can get the url from the provider so we dont have to pass an extra url param here
    rpc_url: Url,
    provider: P,
    world_address: Felt,
    world_diff: &WorldDiff,
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

    let policies = collect_policies(world_address, contract_address, world_diff)?;

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
    world_address: Felt,
    user_address: Felt,
    world_diff: &WorldDiff,
) -> Result<Vec<PolicyMethod>> {
    let policies = collect_policies_from_local_world(world_address, user_address, world_diff)?;
    trace!(target: "account::controller", policies_count = policies.len(), "Extracted policies from project.");
    Ok(policies)
}

fn collect_policies_from_local_world(
    world_address: Felt,
    user_address: Felt,
    world_diff: &WorldDiff,
) -> Result<Vec<PolicyMethod>> {
    let mut policies: Vec<PolicyMethod> = Vec::new();

    // get methods from all project contracts
    for (selector, resource) in world_diff.resources.iter() {
        if resource.resource_type() == ResourceType::Contract {
            // Safe to unwrap the two methods since the selector comes from the resources registry
            // in the local world.
            let contract_address = world_diff.get_contract_address(*selector).unwrap();
            let sierra_class = world_diff.get_class(*selector).unwrap();

            policies_from_abis(&mut policies, &resource.tag(), contract_address, &sierra_class.abi);
        }
    }

    // get method from world contract
    policies_from_abis(&mut policies, "world", world_address, &world_diff.world_info.class.abi);

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

/// Recursively extract methods and convert them into policies from the all the
/// ABIs in the project.
fn policies_from_abis(
    policies: &mut Vec<PolicyMethod>,
    contract_tag: &str,
    contract_address: Felt,
    entries: &[AbiEntry],
) {
    for entry in entries {
        match entry {
            AbiEntry::Function(f) => {
                // we only create policies for non-view functions
                if let StateMutability::External = f.state_mutability {
                    let policy =
                        PolicyMethod { target: contract_address, method: f.name.to_string() };
                    trace!(target: "account::controller", tag = contract_tag, target = format!("{:#x}", policy.target), method = %policy.method, "Adding policy");
                    policies.push(policy);
                }
            }

            AbiEntry::Interface(i) => {
                policies_from_abis(policies, contract_tag, contract_address, &i.items)
            }

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use dojo_test_utils::compiler::CompilerTestSetup;
    use dojo_world::diff::WorldDiff;
    use scarb::compiler::Profile;
    use sozo_scarbext::WorkspaceExt;
    use starknet::macros::felt;

    use super::{collect_policies, PolicyMethod};

    #[test]
    fn collect_policies_from_project() {
        let current_dir = std::env::current_dir().unwrap();
        println!("Current directory: {:?}", current_dir);
        let setup = CompilerTestSetup::from_examples("../../crates/dojo/core", "../../examples/");
        let config = setup.build_test_config("spawn-and-move", Profile::DEV);

        let ws = scarb::ops::read_workspace(config.manifest_path(), &config)
            .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

        let world_local = ws.load_world_local().unwrap();
        let world_diff = WorldDiff::from_local(world_local).unwrap();

        let user_addr = felt!("0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba");

        let policies =
            collect_policies(world_diff.world_info.address, user_addr, &world_diff).unwrap();

        if std::env::var("POLICIES_FIX").is_ok() {
            let policies_json = serde_json::to_string_pretty(&policies).unwrap();
            println!("{}", policies_json);
        } else {
            let test_data = include_str!("../../../../tests/test_data/policies.json");
            let expected_policies: Vec<PolicyMethod> = serde_json::from_str(test_data).unwrap();

            // Compare the collected policies with the test data.
            assert_eq!(policies.len(), expected_policies.len());
            expected_policies.iter().for_each(|p| assert!(policies.contains(p)));
        }
    }
}
