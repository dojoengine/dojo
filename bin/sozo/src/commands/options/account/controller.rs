use account_sdk::account::session::hash::{AllowedMethod, Session};
use account_sdk::account::session::SessionAccount;
use account_sdk::deploy_contract::UDC_ADDRESS;
use account_sdk::signers::HashSigner;
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use dojo_world::manifest::{BaseManifest, DojoContract, Manifest};
use dojo_world::migration::strategy::generate_salt;
use scarb::core::Config;
use slot::session::Policy;
use starknet::core::types::contract::AbiEntry;
use starknet::core::types::FieldElement;
use starknet::core::utils::{cairo_short_string_to_felt, get_contract_address};
use starknet::macros::short_string;
use starknet::providers::Provider;
use starknet::signers::SigningKey;
use starknet_crypto::poseidon_hash_single;
use tracing::trace;
use url::Url;

use super::WorldAddressOrName;

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

    // Check if the session exists, if not create a new one
    let session_details = match slot::session::get(chain_id) {
        // TODO(kariy): perform policies diff check, if needed update
        Ok(Some(session)) => {
            trace!(expires_at = %session.expires_at, policies = session.policies.len(), "Found existing session.");
            session
        }

        // Return error if user not logged in on slot yet
        Err(e @ slot::Error::Unauthorized) => {
            return Err(e.into());
        }

        // Create a new session if not found or other error
        Ok(None) | Err(_) => {
            trace!(%username, chain = format!("{chain_id:#}"), "Creating new session key.");
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

    trace!(
        chain = format!("{chain_id:#x}"),
        address = format!("{contract_address:#x}"),
        "Created Controller session account"
    );

    Ok(session_account)
}

/// Policies are the building block of a session key. It's what defines what methods are allowed for
/// an external signer to execute using the session key.
///
/// This function collect all the contracts' methods in the current project according to the
/// project's deployment manifest (manifest.toml) and convert them into policies.
fn collect_policies(
    world_addr_or_name: WorldAddressOrName,
    user_address: FieldElement,
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
    user_address: FieldElement,
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
        policies_from_abis(&mut policies, &contract.name, contract_address, &abis);
    }

    // get method from world contract
    let abis = manifest.world.inner.abi.unwrap().load_abi_string(&base_path)?;
    let abis = serde_json::from_str::<Vec<AbiEntry>>(&abis)?;
    policies_from_abis(&mut policies, &manifest.world.name, world_address, &abis);

    // for sending declare tx
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
    contract_name: &str,
    contract_address: FieldElement,
    entries: &[AbiEntry],
) {
    for entry in entries {
        match entry {
            AbiEntry::Function(f) => {
                let method = f.name.to_string();
                let policy = Policy { target: contract_address, method };
                trace!(name = contract_name, target = format!("{:#x}", policy.target), method = %policy.method, "Adding policy");
                policies.push(policy);
            }

            AbiEntry::Interface(i) => {
                policies_from_abis(policies, contract_name, contract_address, &i.items)
            }

            _ => {}
        }
    }
}

fn get_dojo_contract_address(
    world_address: FieldElement,
    manifest: &Manifest<DojoContract>,
) -> FieldElement {
    if let Some(address) = manifest.inner.address {
        address
    } else {
        let salt = generate_salt(&manifest.name);
        get_contract_address(salt, manifest.inner.base_class_hash, &[], world_address)
    }
}

fn get_dojo_world_address(
    world_address: WorldAddressOrName,
    manifest: &BaseManifest,
) -> Result<FieldElement> {
    match world_address {
        WorldAddressOrName::Address(addr) => Ok(addr),
        WorldAddressOrName::Name(name) => {
            let seed = cairo_short_string_to_felt(&name).context("Failed to parse World name.")?;
            let salt = poseidon_hash_single(seed);
            let address = get_contract_address(
                salt,
                manifest.world.inner.original_class_hash,
                &[manifest.base.inner.original_class_hash],
                FieldElement::ZERO,
            );
            Ok(address)
        }
    }
}
