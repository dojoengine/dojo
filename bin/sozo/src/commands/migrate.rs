use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use dojo_utils::TxnConfig;
use dojo_world::config::{Environment, ProfileConfig};
use dojo_world::contracts::{WorldContract, WorldContractReader};
use dojo_world::diff::WorldDiff;
use dojo_world::local::WorldLocal;
use dojo_world::remote::WorldRemote;
use katana_rpc_api::starknet::RPC_SPEC_VERSION;
use scarb::core::{Config, Workspace};
use sozo_ops::migrate::{self, deployer, Migration, MigrationError};
use sozo_ops::scarb_extensions::WorkspaceExt;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, Felt, StarknetError};
use starknet::core::utils as snutils;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use tracing::{debug, trace};

use super::options::account::{AccountOptions, SozoAccount};
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[command(flatten)]
    transaction: TransactionOptions,

    #[command(flatten)]
    world: WorldOptions,

    #[command(flatten)]
    starknet: StarknetOptions,

    #[command(flatten)]
    account: AccountOptions,
}

impl MigrateArgs {
    /// Runs the migration.
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        ws.profile_check()?;

        let (_profile_name, profile_config) = utils::load_profile_config(config)?;

        let MigrateArgs { world, starknet, account, .. } = self;

        let world_local = WorldLocal::from_directory(
            ws.target_dir_profile().to_string(),
            profile_config.namespace.clone(),
        )?;

        let (world_address, account) = config.tokio_handle().block_on(async {
            setup_env(account, starknet, world, &profile_config, &world_local).await
        })?;

        config.tokio_handle().block_on(async {
            let mut txn_config: TxnConfig = self.transaction.into();
            txn_config.wait = true;

            let world_diff = if deployer::is_deployed(world_address, &account.provider()).await? {
                let world_remote =
                    WorldRemote::from_events(world_address, &account.provider()).await?;

                WorldDiff::new(world_local, world_remote)
            } else {
                WorldDiff::from_local(world_local)
            };

            let migration = Migration::new(
                world_diff,
                WorldContract::new(world_address, account),
                txn_config,
                profile_config,
            );

            migration.migrate().await.context("Migration failed.")
        })
    }
}

pub async fn setup_env(
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    profile_config: &ProfileConfig,
    world_local: &WorldLocal,
) -> Result<(Felt, SozoAccount<JsonRpcClient<HttpTransport>>)> {
    let env = profile_config.env.as_ref();

    let deterministic_world_address =
        world_local.compute_world_address(&profile_config.world.seed)?;

    // If the world address is not provided, we rely on the deterministic address of the
    // world contract based on the seed. If the world already exists, the user must
    // provide the world's address explicitly.
    let world_address = if let Some(wa) = world.address(env)? {
        wa
    } else {
        debug!(
            "This seems to be a first deployment. If you were expecting to update your remote \
             world, please specify its address using --world, in an environment variable or in \
             the dojo configuration file.\n"
        );

        deterministic_world_address
    };

    let account = {
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

        let chain_id = provider.chain_id().await?;
        let chain_id = snutils::parse_cairo_short_string(&chain_id)
            .with_context(|| "Cannot parse chain_id as string")?;
        trace!(chain_id);

        let account =
            { account.account(provider, world_address, &starknet, env, &world_local).await? };

        let address = account.address();

        debug!(chain_id);

        match account.provider().get_class_hash_at(BlockId::Tag(BlockTag::Pending), address).await {
            Ok(_) => Ok(account),
            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                Err(anyhow!("Account with address {:#x} doesn't exist.", account.address()))
            }
            Err(e) => Err(e.into()),
        }
    }
    .with_context(|| "Problem initializing account for migration.")?;

    Ok((world_address, account))
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
