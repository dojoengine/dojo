use std::path::Path;
use std::str::FromStr;

use account_sdk::account::session::hash::{AllowedMethod, Session};
use account_sdk::account::session::SessionAccount;
use account_sdk::deploy_contract::UDC_ADDRESS;
use account_sdk::signers::HashSigner;
use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_world::manifest::DeploymentManifest;
use scarb::core::Config;
use slot::session::Policy;
use starknet::core::types::contract::AbiEntry;
use starknet::core::types::FieldElement;
use starknet::macros::short_string;
use starknet::providers::Provider;
use starknet::signers::SigningKey;
use tracing::{info, trace};
use url::Url;

pub type ControllerSessionAccount<P> = SessionAccount<P, SigningKey, SigningKey>;

/// Create a new Catridge Controller account based on session key.
#[tracing::instrument(name = "create_controller", skip(rpc_url, provider, config))]
pub async fn create_controller<P>(
    rpc_url: Url,
    provider: P,
    config: &Config,
) -> Result<ControllerSessionAccount<P>>
where
    P: Provider,
    P: Send + Sync,
{
    let chain_id = provider.chain_id().await?;
    let credentials = slot::credential::Credentials::load()?;

    let username = credentials.account.clone().unwrap().id;
    let contract_address =
        FieldElement::from_str(&credentials.account.unwrap().contract_address.unwrap())?;

    let session_details = match slot::session::get(chain_id) {
        Ok(Some(session)) => {
            info!(expires_at = %session.expires_at, policies = session.policies.len(), "Found existing session.");
            // TODO(kariy): perform policies diff check, if needed update
            session
        }
        // TODO(kariy): should handle non authenticated error
        Ok(None) | Err(_) => {
            info!(%username, chain = format!("{chain_id:#}"), "Creating new session key.");

            // Project root dir
            let root_dir = config.root();

            let mut manifest_path = root_dir.to_path_buf();
            manifest_path.extend(["manifests", config.profile().as_str(), "manifest.toml"]);

            info!(path = manifest_path.as_str(), "Extracing policies from project manifest.");

            let manifest = DeploymentManifest::load_from_path(&manifest_path)?;
            let policies = collect_policies(root_dir, manifest, contract_address);

            info!(policies_count = policies.len(), "Extracted policies from project.");

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

    let guardian = SigningKey::from_secret_scalar(short_string!("CARTRIDGE_GUARDIAN"));
    let signer = SigningKey::from_secret_scalar(session_details.credentials.private_key);
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
        chain = format!("{chain_id:#}"),
        address = format!("{contract_address:#x}"),
        "Created Controller session account"
    );

    Ok(session_account)
}

/// Collect all the contracts' methods in the current project and convert them into policies.
fn collect_policies(
    root_dir: impl AsRef<Path>,
    manifest: DeploymentManifest,
    user_address: FieldElement,
) -> Vec<Policy> {
    let mut policies: Vec<Policy> = Vec::new();
    let root_dir: Utf8PathBuf = root_dir.as_ref().to_path_buf().try_into().unwrap();

    // get methods from all project contracts
    for contract in manifest.contracts {
        let abis = contract.inner.abi.unwrap().load_abi_string(&root_dir).unwrap();
        let abis = serde_json::from_str::<Vec<AbiEntry>>(&abis).unwrap();
        let contract_address = contract.inner.address.unwrap();
        policies_from_abis(&mut policies, &contract.name, contract_address, &abis);
    }

    // get method from world contract
    let abis = manifest.world.inner.abi.unwrap().load_abi_string(&root_dir).unwrap();
    let abis = serde_json::from_str::<Vec<AbiEntry>>(&abis).unwrap();
    let contract_address = manifest.world.inner.address.unwrap();
    policies_from_abis(&mut policies, &manifest.world.name, contract_address, &abis);

    // for sending declare tx
    let method = "__declare_transaction__".to_string();
    policies.push(Policy { target: user_address, method });
    info!("Adding declare transaction policy");

    // for deploying using udc
    let method = "deployContract".to_string();
    policies.push(Policy { target: *UDC_ADDRESS, method });
    info!("Adding UDC deployment policy");

    policies
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
                info!(name = contract_name, target = format!("{:#x}", policy.target), method = %policy.method, "Adding policy");
                policies.push(policy);
            }

            AbiEntry::Interface(i) => {
                policies_from_abis(policies, contract_name, contract_address, &i.items)
            }

            _ => {}
        }
    }
}
