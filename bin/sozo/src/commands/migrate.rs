use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use dojo_world::manifest::MANIFESTS_DIR;
use dojo_world::metadata::{dojo_metadata_from_workspace, Environment};
use dojo_world::migration::TxnConfig;
use katana_rpc_api::starknet::RPC_SPEC_VERSION;
use scarb::core::{Config, Workspace};
use sozo_ops::migration;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, FieldElement, StarknetError};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use tracing::trace;

use crate::commands::options::account::WorldAddressOrName;

use super::options::account::{AccountOptions, SozoAccount};
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[command(subcommand)]
    pub command: MigrateCommand,

    #[arg(long, global = true)]
    #[arg(help = "Name of the World.")]
    #[arg(long_help = "Name of the World. It's hash will be used as a salt when deploying the \
                       contract to avoid address conflicts. If not provided root package's name \
                       will be used.")]
    name: Option<String>,

    #[command(flatten)]
    world: WorldOptions,

    #[command(flatten)]
    starknet: StarknetOptions,

    #[command(flatten)]
    account: AccountOptions,
}

#[derive(Debug, Subcommand)]
pub enum MigrateCommand {
    #[command(about = "Plan the migration and output the manifests.")]
    Plan,
    #[command(about = "Apply the migration on-chain.")]
    Apply {
        #[command(flatten)]
        transaction: TransactionOptions,
    },
    #[command(about = "Generate overlays file.")]
    GenerateOverlays,
}

impl MigrateArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let dojo_metadata = if let Some(metadata) = dojo_metadata_from_workspace(&ws) {
            metadata
        } else {
            return Err(anyhow!(
                "No current package with dojo metadata found, migrate is not yet support for \
                 workspaces."
            ));
        };

        // This variant is tested before the match on `self.command` to avoid
        // having the need to spin up a Katana to generate the files.
        if let MigrateCommand::GenerateOverlays = self.command {
            trace!("Generating overlays.");
            return migration::generate_overlays(&ws);
        }

        let env_metadata = if config.manifest_path().exists() {
            dojo_metadata.env().cloned()
        } else {
            trace!("Manifest path does not exist.");
            None
        };

        let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
        if !manifest_dir.join(MANIFESTS_DIR).exists() {
            return Err(anyhow!("Build project using `sozo build` first"));
        }

        let MigrateArgs { name, world, starknet, account, .. } = self;

        let name = name.unwrap_or_else(|| {
            ws.current_package().expect("Root package to be present").id.name.to_string()
        });

        let (world_address, account, rpc_url) = config.tokio_handle().block_on(async {
            setup_env(&ws, account, starknet, world, &name, env_metadata.as_ref()).await
        })?;

        match self.command {
            MigrateCommand::Plan => config.tokio_handle().block_on(async {
                trace!(name, "Planning migration.");
                migration::migrate(
                    &ws,
                    world_address,
                    rpc_url,
                    account,
                    &name,
                    true,
                    TxnConfig::default(),
                    dojo_metadata.skip_migration,
                )
                .await
            }),
            MigrateCommand::Apply { transaction } => config.tokio_handle().block_on(async {
                trace!(name, "Applying migration.");
                let txn_config: TxnConfig = transaction.into();

                migration::migrate(
                    &ws,
                    world_address,
                    rpc_url,
                    account,
                    &name,
                    false,
                    txn_config,
                    dojo_metadata.skip_migration,
                )
                .await
            }),
            _ => unreachable!("other case handled above."),
        }
    }
}

pub async fn setup_env<'a>(
    ws: &'a Workspace<'a>,
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    name: &str,
    env: Option<&'a Environment>,
) -> Result<(Option<FieldElement>, SozoAccount<JsonRpcClient<HttpTransport>>, String)> {
    trace!("Setting up environment.");
    let ui = ws.config().ui();

    let world_address = world.address(env).ok();
    trace!(?world_address);

    let (account, rpc_url) = {
        let provider = starknet.provider(env)?;
        trace!(?provider, "Provider initialized.");

        let spec_version = provider.spec_version().await?;
        trace!(spec_version);

        if !is_compatible_version(&spec_version, RPC_SPEC_VERSION)? {
            return Err(anyhow!(
                "Unsupported Starknet RPC version: {}, expected {}.",
                spec_version,
                RPC_SPEC_VERSION
            ));
        }

        let rpc_url = starknet.url(env)?;
        trace!(?rpc_url);

        let chain_id = provider.chain_id().await?;
        let chain_id = parse_cairo_short_string(&chain_id)
            .with_context(|| "Cannot parse chain_id as string")?;
        trace!(chain_id);

        let account = {
            // This is mainly for controller account for creating policies.
            let world_address_or_name = world_address
                .map(WorldAddressOrName::Address)
                .unwrap_or(WorldAddressOrName::Name(name.to_string()));

            account.account(provider, world_address_or_name, &starknet, env, ws.config()).await?
        };

        let address = account.address();

        ui.print(format!("\nMigration account: {address:#x}"));

        ui.print(format!("\nWorld name: {name}"));

        ui.print(format!("\nChain ID: {chain_id}\n"));

        match account.provider().get_class_hash_at(BlockId::Tag(BlockTag::Pending), address).await {
            Ok(_) => Ok((account, rpc_url)),
            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                Err(anyhow!("Account with address {:#x} doesn't exist.", account.address()))
            }
            Err(e) => Err(e.into()),
        }
    }
    .with_context(|| "Problem initializing account for migration.")?;

    Ok((world_address, account, rpc_url.to_string()))
}

/// Checks if the provided version string is compatible with the expected version string using
/// semantic versioning rules. Includes specific backward compatibility rules, e.g., version 0.6 is
/// compatible with 0.7.
///
/// # Arguments
///
/// * `provided_version` - The version string provided by the user.
/// * `expected_version` - The expected version string.
///
/// # Returns
///
/// * `Result<bool>` - Returns `true` if the provided version is compatible with the expected
///   version, `false` otherwise.
fn is_compatible_version(provided_version: &str, expected_version: &str) -> Result<bool> {
    use semver::{Version, VersionReq};

    let provided_ver = Version::parse(provided_version)
        .map_err(|e| anyhow!("Failed to parse provided version '{}': {}", provided_version, e))?;
    let expected_ver = Version::parse(expected_version)
        .map_err(|e| anyhow!("Failed to parse expected version '{}': {}", expected_version, e))?;

    // Specific backward compatibility rule: 0.6 is compatible with 0.7.
    if (provided_ver.major == 0 && provided_ver.minor == 7)
        && (expected_ver.major == 0 && expected_ver.minor == 6)
    {
        return Ok(true);
    }

    let expected_ver_req = VersionReq::parse(expected_version).map_err(|e| {
        anyhow!("Failed to parse expected version requirement '{}': {}", expected_version, e)
    })?;

    Ok(expected_ver_req.matches(&provided_ver))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_compatible_version_major_mismatch() {
        assert!(!is_compatible_version("1.0.0", "2.0.0").unwrap());
    }

    #[test]
    fn test_is_compatible_version_minor_compatible() {
        assert!(is_compatible_version("1.2.0", "1.1.0").unwrap());
    }

    #[test]
    fn test_is_compatible_version_minor_mismatch() {
        assert!(!is_compatible_version("0.2.0", "0.7.0").unwrap());
    }

    #[test]
    fn test_is_compatible_version_specific_backward_compatibility() {
        let node_version = "0.7.1";
        let katana_version = "0.6.0";
        assert!(is_compatible_version(node_version, katana_version).unwrap());
    }

    #[test]
    fn test_is_compatible_version_invalid_version_string() {
        assert!(is_compatible_version("1.0", "1.0.0").is_err());
        assert!(is_compatible_version("1.0.0", "1.0").is_err());
    }
}
