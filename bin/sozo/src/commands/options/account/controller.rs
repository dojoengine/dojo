use account_sdk::account::session::hash::{AllowedMethod, Session};
use account_sdk::account::session::SessionAccount;
use account_sdk::deploy_contract::UDC_ADDRESS;
use account_sdk::signers::HashSigner;
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use dojo_world::contracts::naming::get_name_from_tag;
use dojo_world::manifest::{BaseManifest, DojoContract, Manifest};
use dojo_world::migration::strategy::generate_salt;
use scarb::core::Config;
use slot::session::Policy;
use starknet::core::types::contract::{AbiEntry, StateMutability};
use starknet::core::types::Felt;
use starknet::core::utils::{cairo_short_string_to_felt, get_contract_address};
use starknet::macros::short_string;
use starknet::providers::Provider;
use starknet::signers::SigningKey;
use starknet_crypto::poseidon_hash_single;
use tracing::trace;
use url::Url;

use super::WorldAddressOrName;

// This type comes from account_sdk, which doesn't derive Debug.
#[allow(missing_debug_implementations)]
pub type ControllerSessionAccount<P> = SessionAccount<P, SigningKey, SigningKey>;

/// Create a new Catridge Controller account based on session key.
#[tracing::instrument(
    name = "create_controller",
    skip(rpc_url, provider, world_addr_or_name, config)
)]
pub async fn create_controller<P>(
    // Ideally we can get the url from the provider so we dont have to pass an extra url param here
    rpc_url: Url,
    provider: P,
    // Use to either specify the world address or compute the world address from the world name
    world_addr_or_name: WorldAddressOrName,
    config: &Config,
) -> Result<ControllerSessionAccount<P>>
where
    P: Provider,
    P: Send + Sync,
{
    let chain_id = provider.chain_id().await?;
    let credentials = slot::credential::Credentials::load()?;

    let username = credentials.account.id;
    let contract_address = credentials.account.contract_address;

    trace!(
        %username,
        chain = format!("{chain_id:#x}"),
        address = format!("{contract_address:#x}"),
        "Creating Controller session account"
    );

    // Check if the session exists, if not create a new one
    let session_details = match slot::session::get(chain_id)? {
        Some(session) => {
            trace!(expires_at = %session.expires_at, policies = session.policies.len(), "Found existing session.");

            // Perform policies diff check. For security reasons, we will always create a new
            // session here if the current policies are different from the existing
            // session. TODO(kariy): maybe don't need to update if current policies is a
            // subset of the existing policies.
            let policies = collect_policies(world_addr_or_name, contract_address, config)?;

            if policies != session.policies {
                trace!(
                    new_policies = policies.len(),
                    existing_policies = session.policies.len(),
                    "Policies have changed. Creating new session."
                );

                let session = slot::session::create(rpc_url, &policies).await?;
                slot::session::store(chain_id, &session)?;
                session
            } else {
                session
            }
        }

        // Create a new session if not found
        None => {
            trace!(%username, chain = format!("{chain_id:#}"), "Creating new session.");
            let policies = collect_policies(world_addr_or_name, contract_address, config)?;
            let session = slot::session::create(rpc_url, &policies).await?;
            slot::session::store(chain_id, &session)?;
            session
        }
    };

    let methods = session_details
        .policies
        .into_iter()
        .map(|p| AllowedMethod::new(p.target, &p.method))
        .collect::<Result<Vec<AllowedMethod>, _>>()?;

    // Copied from `account-wasm` <https://github.com/cartridge-gg/controller/blob/0dd4dd6cbc5fcd3b9a1fd8d63dc127f6312b733f/packages/account-wasm/src/lib.rs#L78-L88>
    let guardian = SigningKey::from_secret_scalar(short_string!("CARTRIDGE_GUARDIAN"));
    let signer = SigningKey::from_secret_scalar(session_details.credentials.private_key);
    // TODO(kariy): make `expires_at` a `u64` type in the session struct
    let expires_at = session_details.expires_at.parse::<u64>()?;
    let session = Session::new(methods, expires_at, &signer.signer())?;

    let session_account = SessionAccount::new(
        provider,
        signer,
        guardian,
        contract_address,
        chain_id,
        session_details.credentials.authorization,
        session,
    );

    Ok(session_account)
}

/// Policies are the building block of a session key. It's what defines what methods are allowed for
/// an external signer to execute using the session key.
///
/// This function collect all the contracts' methods in the current project according to the
/// project's base manifest ( `/manifests/<profile>/base` ) and convert them into policies.
fn collect_policies(
    world_addr_or_name: WorldAddressOrName,
    user_address: Felt,
    config: &Config,
) -> Result<Vec<Policy>> {
    let root_dir = config.root();
    let manifest = get_project_base_manifest(root_dir, config.profile().as_str())?;
    let policies =
        collect_policies_from_base_manifest(world_addr_or_name, user_address, root_dir, manifest)?;
    trace!(policies_count = policies.len(), "Extracted policies from project.");
    Ok(policies)
}

fn get_project_base_manifest(root_dir: &Utf8Path, profile: &str) -> Result<BaseManifest> {
    let mut manifest_path = root_dir.to_path_buf();
    manifest_path.extend(["manifests", profile, "base"]);
    Ok(BaseManifest::load_from_path(&manifest_path)?)
}

fn collect_policies_from_base_manifest(
    world_address: WorldAddressOrName,
    user_address: Felt,
    base_path: &Utf8Path,
    manifest: BaseManifest,
) -> Result<Vec<Policy>> {
    let mut policies: Vec<Policy> = Vec::new();
    let base_path: Utf8PathBuf = base_path.to_path_buf();

    // compute the world address here if it's a name
    let world_address = get_dojo_world_address(world_address, &manifest)?;

    // get methods from all project contracts
    for contract in manifest.contracts {
        let contract_address = get_dojo_contract_address(world_address, &contract);
        let abis = contract.inner.abi.unwrap().load_abi_string(&base_path)?;
        let abis = serde_json::from_str::<Vec<AbiEntry>>(&abis)?;
        policies_from_abis(&mut policies, &contract.inner.tag, contract_address, &abis);
    }

    // get method from world contract
    let abis = manifest.world.inner.abi.unwrap().load_abi_string(&base_path)?;
    let abis = serde_json::from_str::<Vec<AbiEntry>>(&abis)?;
    policies_from_abis(&mut policies, "world", world_address, &abis);

    // special policy for sending declare tx
    // corresponds to [account_sdk::account::DECLARATION_SELECTOR]
    let method = "__declare_transaction__".to_string();
    policies.push(Policy { target: user_address, method });
    trace!("Adding declare transaction policy");

    // for deploying using udc
    let method = "deployContract".to_string();
    policies.push(Policy { target: *UDC_ADDRESS, method });
    trace!("Adding UDC deployment policy");

    Ok(policies)
}

/// Recursively extract methods and convert them into policies from the all the
/// ABIs in the project.
fn policies_from_abis(
    policies: &mut Vec<Policy>,
    contract_tag: &str,
    contract_address: Felt,
    entries: &[AbiEntry],
) {
    for entry in entries {
        match entry {
            AbiEntry::Function(f) => {
                // we only create policies for non-view functions
                if let StateMutability::External = f.state_mutability {
                    let policy = Policy { target: contract_address, method: f.name.to_string() };
                    trace!(tag = contract_tag, target = format!("{:#x}", policy.target), method = %policy.method, "Adding policy");
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

fn get_dojo_contract_address(world_address: Felt, manifest: &Manifest<DojoContract>) -> Felt {
    if let Some(address) = manifest.inner.address {
        address
    } else {
        let salt = generate_salt(&get_name_from_tag(&manifest.inner.tag));
        get_contract_address(salt, manifest.inner.base_class_hash, &[], world_address)
    }
}

fn get_dojo_world_address(
    world_address: WorldAddressOrName,
    manifest: &BaseManifest,
) -> Result<Felt> {
    match world_address {
        WorldAddressOrName::Address(addr) => Ok(addr),
        WorldAddressOrName::Name(name) => {
            let seed = cairo_short_string_to_felt(&name).context("Failed to parse World name.")?;
            let salt = poseidon_hash_single(seed);
            let address = get_contract_address(
                salt,
                manifest.world.inner.original_class_hash,
                &[manifest.base.inner.original_class_hash],
                Felt::ZERO,
            );
            Ok(address)
        }
    }
}
