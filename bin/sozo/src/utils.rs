use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use colored::*;
use dojo_utils::provider as provider_utils;
use dojo_world::config::ProfileConfig;
use dojo_world::contracts::ContractInfo;
use dojo_world::diff::WorldDiff;
use dojo_world::local::WorldLocal;
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use semver::{Version, VersionReq};
use sozo_ops::migration_ui::MigrationUi;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;
use starknet::core::utils as snutils;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tracing::trace;

use crate::commands::options::account::{AccountOptions, SozoAccount};
use crate::commands::options::starknet::StarknetOptions;
use crate::commands::options::world::WorldOptions;

/// The maximum number of blocks that will separate the `from_block` and the `to_block` in the
/// event fetching, which if too high will cause the event fetching to fail in most of the node
/// providers.
pub const MAX_BLOCK_RANGE: u64 = 200_000;

pub const RPC_SPEC_VERSION: &str = "0.8.1";

pub const CALLDATA_DOC: &str = "
Space separated values e.g., 0x12345 128 u256:9999999999 str:'hello world'.
Sozo supports some prefixes that you can use to automatically parse some types. The supported \
                                prefixes are:
    - u256: A 256-bit unsigned integer.
    - sstr: A cairo short string.
            If the string contains spaces it must be between quotes (ex: sstr:'hello world')
    - str: A cairo string (ByteArray).
            If the string contains spaces it must be between quotes (ex: sstr:'hello world')
    - int: A signed integer.
    - arr: A dynamic array where each item fits on a single felt252.
    - u256arr: A dynamic array of u256.
    - farr: A fixed-size array where each item fits on a single felt252.
    - u256farr: A fixed-size array of u256.
    - no prefix: A cairo felt or any type that fit into one felt.";

// Computes the world address based on the provided options.
pub fn get_world_address(
    profile_config: &ProfileConfig,
    world: &WorldOptions,
    world_local: &WorldLocal,
) -> Result<Felt> {
    let env = profile_config.env.as_ref();

    let deterministic_world_address = world_local.deterministic_world_address()?;

    if let Some(wa) = world.address(env)? {
        if wa != deterministic_world_address && !world.guest {
            println!(
                "{}",
                format!(
                    "The world address computed from the seed is different from the address \
                     provided in config:\n\ndeterministic address: {:#x}\nconfig address: \
                     {:#x}\n\nThe address in the config file is preferred, consider commenting it \
                     out from the config file if you attempt to migrate the world with a new \
                     seed.\n\nIf you are upgrading the world, you can ignore this message.",
                    deterministic_world_address, wa
                )
                .yellow()
            );
        }

        Ok(wa)
    } else {
        Ok(deterministic_world_address)
    }
}

// Sets up the world diff from the environment and returns associated starknet account.
//
// Returns the world address, the world diff, the starknet provider and the rpc url.
pub async fn get_world_diff_and_provider(
    starknet: StarknetOptions,
    world: WorldOptions,
    scarb_metadata: &Metadata,
) -> Result<(WorldDiff, JsonRpcClient<HttpTransport>, String)> {
    let world_local = scarb_metadata.load_dojo_world_local()?;
    let profile_config = scarb_metadata.load_dojo_profile_config()?;

    let env = profile_config.env.as_ref();

    let world_address = get_world_address(&profile_config, &world, &world_local)?;

    let (provider, rpc_url) = starknet.provider(env)?;
    let provider = Arc::new(provider);
    if (provider_utils::health_check_provider(provider.clone()).await).is_err() {
        //         warn!(
        // "provider health check failed during sozo inspect, inspecting locally
        // and all resources will appeared as `created`. remote resources will not be fetched."
        // );
        return Ok((
            WorldDiff::from_local(world_local)?,
            Arc::try_unwrap(provider).map_err(|_| anyhow!("Failed to unwrap Arc"))?,
            rpc_url,
        ));
    }

    let provider = Arc::try_unwrap(provider).map_err(|_| anyhow!("Failed to unwrap Arc"))?;
    trace!(?provider, "Provider initialized.");

    let spec_version = provider.spec_version().await?;
    trace!(spec_version);

    if !is_compatible_version(&spec_version, RPC_SPEC_VERSION)? {
        return Err(anyhow!(
            "Unsupported Starknet RPC version: {spec_version}, expected {RPC_SPEC_VERSION}.",
        ));
    }

    let chain_id = provider.chain_id().await?;
    let chain_id = snutils::parse_cairo_short_string(&chain_id)
        .with_context(|| "Cannot parse chain_id as string")?;
    trace!(chain_id);

    let world_diff = WorldDiff::new_from_chain(
        world_address,
        world_local,
        &provider,
        env.and_then(|e| e.world_block),
        env.and_then(|e| e.max_block_range).unwrap_or(MAX_BLOCK_RANGE),
        &world.namespaces,
    )
    .await?;

    Ok((world_diff, provider, rpc_url))
}

// Sets up the world diff from the environment and returns associated starknet account.
//
// Returns the world address, the world diff, the account and the rpc url.
// This would be convenient to have the rpc url retrievable from the [`Provider`] trait.
pub async fn get_world_diff_and_account(
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    scarb_metadata: &Metadata,
    ui: &mut Option<&mut MigrationUi>,
) -> Result<(WorldDiff, SozoAccount<JsonRpcClient<HttpTransport>>, String)> {
    let profile_config = scarb_metadata.load_dojo_profile_config()?;
    let env = profile_config.env.as_ref();

    let (world_diff, provider, rpc_url) =
        get_world_diff_and_provider(starknet.clone(), world, scarb_metadata).await?;

    // Ensures we don't interfere with the spinner if a password must be prompted.
    if let Some(ui) = ui {
        ui.stop();
    }

    let contracts = (&world_diff).into();

    let account = { account.account(provider, env, &starknet, &contracts).await? };

    if let Some(ui) = ui {
        ui.restart("Verifying account...");
    }

    if !dojo_utils::is_deployed(account.address(), &account.provider()).await? {
        return Err(anyhow!("Account with address {:#x} doesn't exist.", account.address()));
    }

    Ok((world_diff, account, rpc_url))
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

// Returns the contracts from the manifest or from the diff.
#[allow(clippy::unnecessary_unwrap)]
pub async fn contracts_from_manifest_or_diff(
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    scarb_metadata: &Metadata,
    force_diff: bool,
) -> Result<HashMap<String, ContractInfo>> {
    let local_manifest = scarb_metadata.read_dojo_manifest_profile()?;

    let contracts: HashMap<String, ContractInfo> = if force_diff || local_manifest.is_none() {
        let (world_diff, _, _) =
            get_world_diff_and_account(account, starknet, world, scarb_metadata, &mut None).await?;
        (&world_diff).into()
    } else {
        let local_manifest = local_manifest.unwrap();
        (&local_manifest).into()
    };

    Ok(contracts)
}
// Prompts the user to confirm an operation.
pub fn prompt_confirm(prompt: &str) -> Result<bool> {
    print!("{} [y/N]", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_lowercase() == "y")
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
