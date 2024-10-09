use std::str::FromStr;

use anyhow::{Error, Result};
use camino::Utf8PathBuf;
use dojo_world::config::Environment;
use dojo_world::contracts::world::WorldContract;
use dojo_world::contracts::WorldContractReader;
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::{Config, TomlManifest};
use semver::Version;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;

use crate::commands::options::account::{AccountOptions, SozoAccount, WorldAddressOrName};
use crate::commands::options::starknet::StarknetOptions;
use crate::commands::options::world::WorldOptions;

/// Load metadata from the Scarb configuration.
///
/// # Arguments
///
/// * `config` - Scarb project configuration.
///
/// # Returns
///
/// A [`Environment`] on success.
pub fn load_metadata_from_config(config: &Config) -> Result<Option<Environment>, Error> {
    let env_metadata = if config.manifest_path().exists() {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let dojo_metadata = dojo_metadata_from_workspace(&ws)?;

        dojo_metadata.env().cloned()
    } else {
        None
    };

    Ok(env_metadata)
}

/// Build a world contract from the provided environment.
///
/// # Arguments
///
/// * `world` - The world options such as the world address,
/// * `account` - The account options,
/// * `starknet` - The Starknet options such as the RPC url,
/// * `env_metadata` - Optional environment coming from Scarb configuration.
///
/// # Returns
///
/// A [`WorldContract`] on success.
pub async fn world_from_env_metadata(
    world: WorldOptions,
    account: AccountOptions,
    starknet: &StarknetOptions,
    env_metadata: &Option<Environment>,
    config: &Config,
) -> Result<WorldContract<SozoAccount<JsonRpcClient<HttpTransport>>>, Error> {
    let env_metadata = env_metadata.as_ref();

    let world_address = world.address(env_metadata)?;
    let provider = starknet.provider(env_metadata)?;
    let account = account
        .account(
            provider,
            WorldAddressOrName::Address(world_address),
            starknet,
            env_metadata,
            config,
        )
        .await?;

    Ok(WorldContract::new(world_address, account))
}

/// Build a world contract reader from the provided environment.
///
/// # Arguments
///
/// * `world` - The world options such as the world address,
/// * `starknet` - The Starknet options such as the RPC url,
/// * `env_metadata` - Optional environment coming from Scarb configuration.
///
/// # Returns
///
/// A [`WorldContractReader`] on success.
pub async fn world_reader_from_env_metadata(
    world: WorldOptions,
    starknet: StarknetOptions,
    env_metadata: &Option<Environment>,
) -> Result<WorldContractReader<JsonRpcClient<HttpTransport>>, Error> {
    let world_address = world.address(env_metadata.as_ref())?;
    let provider = starknet.provider(env_metadata.as_ref())?;

    Ok(WorldContractReader::new(world_address, provider))
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
            "Specified cairo version not supported by dojo. Please verify and update dojo."
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
