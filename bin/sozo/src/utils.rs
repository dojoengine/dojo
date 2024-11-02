use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use colored::*;
use dojo_world::config::ProfileConfig;
use dojo_world::diff::WorldDiff;
use dojo_world::local::WorldLocal;
use katana_rpc_api::starknet::RPC_SPEC_VERSION;
use scarb::core::{TomlManifest, Workspace};
use semver::Version;
use sozo_ops::migration_ui::MigrationUi;
use sozo_scarbext::WorkspaceExt;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;
use starknet::core::utils as snutils;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tracing::trace;

use crate::commands::options::account::{AccountOptions, SozoAccount};
use crate::commands::options::starknet::StarknetOptions;
use crate::commands::options::world::WorldOptions;

/// Computes the world address based on the provided options.
pub fn get_world_address(
    profile_config: &ProfileConfig,
    world: &WorldOptions,
    world_local: &WorldLocal,
) -> Result<Felt> {
    let env = profile_config.env.as_ref();

    let deterministic_world_address = world_local.deterministic_world_address()?;

    if let Some(wa) = world.address(env)? {
        if wa != deterministic_world_address {
            println!(
                "{}",
                format!(
                    "The world address computed from the seed is different from the address \
                     provided in config:\n\ndeterministic address: {:#x}\nconfig address: \
                     {:#x}\n\nThe address in the config file is preferred, consider commenting it \
                     out if you attempt to migrate the world with a new seed.",
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

pub fn verify_cairo_version_compatibility(manifest_path: &Utf8PathBuf) -> Result<()> {
    let scarb_cairo_version = scarb::version::get().cairo;
    // When manifest file doesn't exists ignore it. Would be the case during `sozo init`
    let Ok(manifest) = TomlManifest::read_from_path(manifest_path) else { return Ok(()) };

    // For any kind of error, like package not specified, cairo version not specified return
    // without an error
    let Some(package) = manifest.package else { return Ok(()) };

    let Some(cairo_version) = package.cairo_version else { return Ok(()) };

    // only when cairo version is found in manifest file confirm that it matches
    let version_req = cairo_version.as_defined().unwrap();
    let version = Version::from_str(scarb_cairo_version.version).unwrap();
    if !version_req.matches(&version) {
        anyhow::bail!(
            "Cairo version {} found in {} is not supported by dojo (expecting {}). Please change \
             the Cairo version in your manifest or update dojo.",
            version_req,
            manifest_path,
            version,
        );
    };

    Ok(())
}

pub fn generate_version() -> String {
    const DOJO_VERSION: &str = env!("CARGO_PKG_VERSION");
    let scarb_version = scarb::version::get().version;
    let scarb_sierra_version = scarb::version::get().sierra.version;
    let scarb_cairo_version = scarb::version::get().cairo.version;

    let version_string = format!(
        "{}\nscarb: {}\ncairo: {}\nsierra: {}",
        DOJO_VERSION, scarb_version, scarb_cairo_version, scarb_sierra_version,
    );
    version_string
}

pub fn is_address(tag_or_address: &str) -> bool {
    tag_or_address.starts_with("0x")
}

/// Sets up the world diff from the environment and returns associated starknet account.
///
/// Returns the world address, the world diff, the starknet provider and the rpc url.
pub async fn get_world_diff_and_provider(
    starknet: StarknetOptions,
    world: WorldOptions,
    ws: &Workspace<'_>,
) -> Result<(WorldDiff, JsonRpcClient<HttpTransport>, String)> {
    let world_local = ws.load_world_local()?;
    let profile_config = ws.load_profile_config()?;

    let env = profile_config.env.as_ref();

    let world_address = get_world_address(&profile_config, &world, &world_local)?;

    let (provider, rpc_url) = starknet.provider(env)?;
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

    let world_diff = WorldDiff::new_from_chain(world_address, world_local, &provider).await?;

    Ok((world_diff, provider, rpc_url))
}

/// Sets up the world diff from the environment and returns associated starknet account.
///
/// Returns the world address, the world diff, the account and the rpc url.
/// This would be convenient to have the rpc url retrievable from the [`Provider`] trait.
pub async fn get_world_diff_and_account(
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    ws: &Workspace<'_>,
    ui: &mut Option<&mut MigrationUi>,
) -> Result<(WorldDiff, SozoAccount<JsonRpcClient<HttpTransport>>, String)> {
    let profile_config = ws.load_profile_config()?;
    let env = profile_config.env.as_ref();

    let (world_diff, provider, rpc_url) =
        get_world_diff_and_provider(starknet.clone(), world, ws).await?;

    // Ensures we don't interfere with the spinner if a password must be prompted.
    if let Some(ui) = ui {
        ui.stop();
    }

    let account = {
        account
            .account(provider, world_diff.world_info.address, &starknet, env, &world_diff)
            .await?
    };

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
