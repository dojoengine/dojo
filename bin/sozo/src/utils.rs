use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use dojo_utils::provider as provider_utils;
use dojo_world::ResourceType;
use dojo_world::config::ProfileConfig;
use dojo_world::contracts::ContractInfo;
use dojo_world::diff::WorldDiff;
use dojo_world::local::WorldLocal;
use scarb_interop::Scarb;
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use semver::{Version, VersionReq};
use sozo_ui::SozoUi;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::Felt;
use starknet::core::utils as snutils;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tracing::trace;

use crate::commands::options::account::{AccountOptions, SozoAccount};
use crate::commands::options::starknet::StarknetOptions;
use crate::commands::options::world::WorldOptions;

/// The maximum number of blocks that will separate the `from_block` and the `to_block` in the
/// event fetching, which if too high will cause the event fetching to fail in most of the node
/// providers.
pub const MAX_BLOCK_RANGE: u64 = 200_000;

pub const RPC_SPEC_VERSION: &str = "0.9.0-rc.2";

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

#[derive(Tabled)]
struct ResourceDetails {
    #[tabled(rename = "Resource Type")]
    resource_type: ResourceType,
    #[tabled(rename = "Tag")]
    tag: String,
    #[tabled(rename = "Selector")]
    selector: String,
    #[tabled(rename = "Status")]
    status: String,
}

// Computes the world address based on the provided options.
pub fn get_world_address(
    profile_config: &ProfileConfig,
    world: &WorldOptions,
    world_local: &WorldLocal,
    ui: &SozoUi,
) -> Result<Felt> {
    let env = profile_config.env.as_ref();

    let deterministic_world_address = world_local.deterministic_world_address()?;

    if let Some(wa) = world.address(env)? {
        if wa != deterministic_world_address && !world.guest {
            ui.new_line();
            ui.warn_block(format!(
                "warning: The world address computed from the seed is different from the address \
                     provided in config:\n  - deterministic address: {:#066x}\n  - config address       : \
                     {:#066x}\n\nThe address in the config file is preferred, consider commenting \
                     it out from the config file if you attempt to migrate the world with a new \
                     seed.\nIf you are upgrading the world, you can ignore this message.",
                deterministic_world_address, wa
            ));
            ui.new_line();
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
    ui: &SozoUi,
) -> Result<(WorldDiff, JsonRpcClient<HttpTransport>, String)> {
    let world_local = scarb_metadata.load_dojo_world_local()?;
    let profile_config = scarb_metadata.load_dojo_profile_config()?;

    let env = profile_config.env.as_ref();

    let world_address = get_world_address(&profile_config, &world, &world_local, ui)?;

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
    ui: &SozoUi,
) -> Result<(WorldDiff, SozoAccount<JsonRpcClient<HttpTransport>>, String)> {
    ui.step("Compute world diff");
    let step_ui = ui.subsection();

    let profile_config = scarb_metadata.load_dojo_profile_config()?;
    let env = profile_config.env.as_ref();

    let (world_diff, provider, rpc_url) =
        get_world_diff_and_provider(starknet.clone(), world, scarb_metadata, &step_ui).await?;

    show_world_details(&profile_config, &world_diff, &step_ui);

    ui.result("World diff computed.");

    let contracts = (&world_diff).into();

    let account = { account.account(provider, env, &starknet, &contracts).await? };

    ui.step("Verify account");

    if !dojo_utils::is_deployed(account.address(), &account.provider()).await? {
        return Err(anyhow!("Account with address {:#066x} doesn't exist.", account.address()));
    }

    ui.result("Account verified.");

    Ok((world_diff, account, rpc_url))
}

fn show_profile_details(profile_config: &ProfileConfig, ui: &SozoUi) {
    ui.verbose("local profile");
    let local_ui = ui.subsection();

    local_ui.verbose(format!(
        "world: (seed: {}, name: {})",
        profile_config.world.seed, profile_config.world.name
    ));

    local_ui.verbose(format!("default namespace: {}", profile_config.namespace.default));

    if let Some(mappings) = profile_config.namespace.mappings.as_ref() {
        local_ui.debug(format!("namespace mappings:"));
        for (namespace, names) in mappings {
            local_ui.debug(format!("   {}: {}", namespace, names.join(", ")));
        }
    }

    if let Some(models) = profile_config.models.as_ref() {
        local_ui.verbose(format!("models: {}", models.len()));
        for model in models {
            local_ui.debug(format!("   {}", model.tag));
        }
    }

    if let Some(contracts) = profile_config.contracts.as_ref() {
        local_ui.verbose(format!("contracts: {}", contracts.len()));
        for contract in contracts {
            local_ui.debug(format!("   {}", contract.tag));
        }
    }

    if let Some(events) = profile_config.events.as_ref() {
        local_ui.verbose(format!("events: {}", events.len()));
        for event in events {
            local_ui.debug(format!("   {}", event.tag));
        }
    }

    if let Some(libraries) = profile_config.libraries.as_ref() {
        local_ui.verbose(format!("libraries: {}", libraries.len()));
        for library in libraries {
            local_ui.debug(format!("   {}", library.tag));
        }
    }

    if let Some(external_contracts) = profile_config.external_contracts.as_ref() {
        local_ui.verbose(format!("external contracts: {}", external_contracts.len()));
        for external_contract in external_contracts {
            let instance_name = external_contract
                .instance_name
                .as_ref()
                .map(|x| format!(" (instance name: {})", x))
                .unwrap_or(String::new());
            local_ui.debug(format!(
                "   contract_name: {}{}",
                external_contract.contract_name, instance_name
            ));
        }
    }

    if let Some(migration) = profile_config.migration.as_ref() {
        local_ui.debug(format!("migration config:"));
        local_ui.debug(format!(
            "   skip_contracts: {}",
            migration.skip_contracts.as_ref().unwrap_or(&Vec::new()).join(", ")
        ));
        local_ui.debug(format!(
            "   disable_multicall: {}",
            migration.disable_multicall.unwrap_or(false)
        ));
        local_ui.debug(format!(
            "   order_inits: {}",
            migration.order_inits.as_ref().unwrap_or(&Vec::new()).join(", ")
        ));
    }

    if let Some(writers) = profile_config.writers.as_ref() {
        local_ui.debug("writers:");
        for (name, tags) in writers {
            local_ui.debug(format!(
                "   {}: {}",
                name,
                tags.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
            ));
        }
    }

    if let Some(owners) = profile_config.owners.as_ref() {
        local_ui.debug("owners:");
        for (name, tags) in owners {
            local_ui.debug(format!(
                "   {}: {}",
                name,
                tags.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
            ));
        }
    }

    if let Some(init_call_args) = profile_config.init_call_args.as_ref() {
        local_ui.debug("init call args:");
        for (name, tags) in init_call_args {
            local_ui.debug(format!(
                "   {}: {}",
                name,
                tags.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
            ));
        }
    }

    if let Some(lib_versions) = profile_config.lib_versions.as_ref() {
        local_ui.debug("lib versions:");
        for (name, version) in lib_versions {
            local_ui.debug(format!("   {}: {}", name, version));
        }
    }

    if let Some(env) = profile_config.env.as_ref() {
        local_ui.debug("environment:");
        local_ui.debug(format!(
            "   account_address: {}",
            env.account_address.as_ref().unwrap_or(&"Not set".to_string())
        ));
        local_ui.debug(format!(
            "   world_address: {}",
            env.world_address.as_ref().unwrap_or(&"Not set".to_string())
        ));
        local_ui.debug(format!(
            "   world_block: {}",
            env.world_block.as_ref().map(|x| x.to_string()).unwrap_or("None".to_string())
        ));
        local_ui.debug(format!(
            "   max_block_range: {}",
            env.max_block_range.as_ref().map(|x| x.to_string()).unwrap_or("None".to_string())
        ));
        if let Some(http_headers) = env.http_headers.as_ref() {
            local_ui.debug("   http_headers:");
            for header in http_headers {
                local_ui.debug(format!("      name: {}, value: {}", header.name, header.value));
            }
        } else {
            local_ui.debug("   http_headers: None");
        }

        if let Some(ipfs_config) = env.ipfs_config.as_ref() {
            local_ui.debug("   ipfs_config:");
            local_ui.debug(format!("      username: {}", ipfs_config.username));
            local_ui.debug(format!("      url: {}", ipfs_config.url));
        } else {
            local_ui.debug("   ipfs_config: None");
        }
    }
}

fn show_world_diff_details(world_diff: &WorldDiff, ui: &SozoUi) {
    ui.verbose("world diff");
    let local_ui = ui.subsection();

    local_ui.verbose(format!("world status: {}", world_diff.world_info.status));
    local_ui.verbose(format!("world address: {:#066x}", world_diff.world_info.address));
    local_ui.debug(format!("world class hash: {:#066x}", world_diff.world_info.class_hash));
    local_ui
        .debug(format!("world casm class hash: {:#066x}", world_diff.world_info.casm_class_hash));

    local_ui.debug("world entrypoints:");
    for entrypoint in &world_diff.world_info.entrypoints {
        local_ui.debug(format!("   {}", entrypoint));
    }

    local_ui.verbose(format!("namespaces: {}", world_diff.namespaces.len()));
    for namespace in &world_diff.namespaces {
        local_ui.debug(format!("   {:#066x}", namespace));
    }

    local_ui.verbose(format!("resources: {}", world_diff.resources.len()));
    let resources = world_diff
        .resources
        .iter()
        .map(|(selector, resource)| ResourceDetails {
            resource_type: resource.resource_type(),
            tag: resource.tag(),
            status: resource.status(),
            selector: selector.to_string(),
        })
        .collect::<Vec<_>>();
    local_ui.debug_block(format!("{}", Table::new(resources).with(Style::psql())));

    local_ui.verbose(format!("external writers: {}", world_diff.external_writers.len()));
    for (selector, writers) in &world_diff.external_writers {
        local_ui.debug(format!("   {:#066x}:", selector));
        for writer in writers {
            local_ui.debug(format!("      {:#066x}", writer));
        }
    }

    local_ui.verbose(format!("external owners: {}", world_diff.external_owners.len()));
    for (selector, owners) in &world_diff.external_owners {
        local_ui.debug(format!("   {:#066x}:", selector));
        for owner in owners {
            local_ui.debug(format!("      {:#066x}", owner));
        }
    }
}

fn show_world_details(profile_config: &ProfileConfig, world_diff: &WorldDiff, ui: &SozoUi) {
    show_profile_details(profile_config, ui);
    show_world_diff_details(world_diff, ui);
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

pub fn generate_version() -> String {
    const DOJO_VERSION: &str = env!("CARGO_PKG_VERSION");

    let scarb_version = if let Some(scarb) = Scarb::version() {
        scarb
    } else {
        "not found in your PATH\n".to_string()
    };

    format!("{}\nscarb: {}", DOJO_VERSION, scarb_version)
}

// Returns the contracts from the manifest or from the diff.
#[allow(clippy::unnecessary_unwrap)]
pub async fn contracts_from_manifest_or_diff(
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    scarb_metadata: &Metadata,
    force_diff: bool,
    ui: &SozoUi,
) -> Result<HashMap<String, ContractInfo>> {
    let local_manifest = scarb_metadata.read_dojo_manifest_profile()?;

    let contracts: HashMap<String, ContractInfo> = if force_diff || local_manifest.is_none() {
        let (world_diff, _, _) =
            get_world_diff_and_account(account, starknet, world, scarb_metadata, ui).await?;
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
