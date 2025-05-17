use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use anyhow::{anyhow, Result};
use cainome::cairo_serde::ContractAddress;
use clap::{Args, Subcommand};
use colored::Colorize;
use dojo_utils::Invoker;
use dojo_world::config::ProfileConfig;
use dojo_world::constants::WORLD;
use dojo_world::contracts::{ContractInfo, WorldContract};
use dojo_world::diff::{DiffPermissions, WorldDiff};
use scarb_interop::MetadataDojoExt;
use scarb_metadata::Metadata;
use sozo_ops::migration_ui::MigrationUi;
use starknet::core::types::Felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tracing::trace;

use super::options::account::{AccountOptions, SozoAccount};
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    #[command(about = "Grant an auth role.")]
    Grant {
        #[command(subcommand)]
        kind: AuthKind,

        #[command(flatten)]
        common: CommonAuthOptions,
    },
    #[command(about = "Revoke an auth role.")]
    Revoke {
        #[command(subcommand)]
        kind: AuthKind,

        #[command(flatten)]
        common: CommonAuthOptions,
    },
    #[command(about = "List the permissions.")]
    List {
        #[arg(help = "The tag of the resource to inspect. If not provided, a world summary will \
                      be displayed.")]
        resource: Option<String>,

        #[arg(
            long,
            help = "Print the address of the grantees, by default only the tag is printed."
        )]
        show_address: bool,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        world: WorldOptions,
    },
    #[command(about = "Clone all permissions that one contract has to another.")]
    Clone {
        #[arg(help = "The tag or address of the source contract to clone the permissions from.")]
        #[arg(long)]
        #[arg(global = true)]
        from: String,

        #[arg(help = "The tag or address of the target contract to clone the permissions to.")]
        #[arg(long)]
        #[arg(global = true)]
        to: String,

        #[arg(
            long,
            help = "Revoke the permissions from the source contract after cloning them to the \
                    target contract."
        )]
        #[arg(global = true)]
        revoke_from: bool,

        #[command(flatten)]
        common: CommonAuthOptions,
    },
}

#[derive(Debug, Args)]
pub struct CommonAuthOptions {
    #[command(flatten)]
    world: WorldOptions,

    #[command(flatten)]
    starknet: StarknetOptions,

    #[command(flatten)]
    account: AccountOptions,

    #[command(flatten)]
    transaction: TransactionOptions,
}

#[derive(Debug, Subcommand)]
pub enum AuthKind {
    #[command(about = "Grant to a contract the permission to write to a resource.")]
    Writer {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "resource_tag,contract_tag_or_address")]
        #[arg(help = "A list of resource/contract couples to grant write access to.
Comma separated values to indicate resource identifier and contract tag or address.\n
Some examples:
   ns-Moves,0x1234
   ns,ns-actions
")]
        pairs: Vec<PermissionPair>,
    },

    #[command(about = "Grant to a contract the ownership of a resource.")]
    Owner {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "resource_tag,contract_tag_or_address")]
        #[arg(help = "A list of resources and owners to grant ownership to.
Comma separated values to indicate resource identifier and owner address.\n
Some examples:
   ns-Moves,ns-actions
   ns,0xbeef
")]
        pairs: Vec<PermissionPair>,
    },
}

impl AuthArgs {
    pub fn run(self, scarb_metadata: &Metadata) -> Result<()> {
        trace!(args = ?self);

        let profile_config = scarb_metadata.load_dojo_profile_config()?;

        let mut rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            match self.command {
                AuthCommand::Grant { kind, common, .. } => {
                    let contracts = utils::contracts_from_manifest_or_diff(
                        common.account.clone(),
                        common.starknet.clone(),
                        common.world.clone(),
                        &scarb_metadata,
                        false,
                    )
                    .await?;

                    let do_grant = true;

                    match kind {
                        AuthKind::Writer { pairs } => {
                            update_writers(&contracts, &common, &profile_config, pairs, do_grant)
                                .await?;
                        }
                        AuthKind::Owner { pairs } => {
                            update_owners(&contracts, &common, &profile_config, pairs, do_grant)
                                .await?;
                        }
                    }
                }
                AuthCommand::Revoke { kind, common, .. } => {
                    let contracts = utils::contracts_from_manifest_or_diff(
                        common.account.clone(),
                        common.starknet.clone(),
                        common.world.clone(),
                        scarb_metadata,
                        false,
                    )
                    .await?;

                    let do_grant = false;

                    match kind {
                        AuthKind::Writer { pairs } => {
                            update_writers(&contracts, &common, &profile_config, pairs, do_grant)
                                .await?;
                        }
                        AuthKind::Owner { pairs } => {
                            update_owners(&contracts, &common, &profile_config, pairs, do_grant)
                                .await?;
                        }
                    }
                }
                AuthCommand::List { resource, show_address, starknet, world } => {
                    list_permissions(resource, show_address, starknet, world, scarb_metadata)
                        .await?;
                }
                AuthCommand::Clone { revoke_from, common, from, to } => {
                    if from == to {
                        anyhow::bail!(
                            "Source and target are the same, please specify different source and \
                             target."
                        );
                    }

                    clone_permissions(common, scarb_metadata, revoke_from, from, to).await?;
                }
            };

            Ok(())
        })
    }
}

/// Clones the permissions from the source contract address to the target contract address.
async fn clone_permissions(
    options: CommonAuthOptions,
    scarb_metadata: &Metadata,
    revoke_from: bool,
    from_tag_or_address: String,
    to_tag_or_address: String,
) -> Result<()> {
    let mut migration_ui = MigrationUi::new_with_frames(
        "Gathering permissions from the world...",
        vec!["🌍", "🔍", "📜"],
    );

    let (world_diff, account, _) = utils::get_world_diff_and_account(
        options.account,
        options.starknet,
        options.world,
        scarb_metadata,
        &mut Some(&mut migration_ui),
    )
    .await?;

    let from_address = resolve_address_or_tag(&from_tag_or_address, &world_diff)?;
    let to_address = resolve_address_or_tag(&to_tag_or_address, &world_diff)?;

    let external_writer_of: Vec<Felt> =
        world_diff
            .external_writers
            .iter()
            .filter_map(|(resource_selector, writers)| {
                if writers.contains(&from_address) { Some(*resource_selector) } else { None }
            })
            .collect();

    let external_owner_of: Vec<Felt> =
        world_diff
            .external_owners
            .iter()
            .filter_map(|(resource_selector, owners)| {
                if owners.contains(&from_address) { Some(*resource_selector) } else { None }
            })
            .collect();

    let mut writer_of = HashSet::new();
    let mut owner_of = HashSet::new();

    for (selector, resource) in world_diff.resources.iter() {
        let writers = world_diff.get_writers(*selector);
        let owners = world_diff.get_owners(*selector);

        if writers.is_empty() && owners.is_empty() {
            continue;
        }

        // We need to check remote only resources if we want to be exhaustive.
        // But in this version, only synced permissions are supported.

        if writers.synced().iter().any(|w| w.address == from_address) {
            writer_of.insert(resource.tag().clone());
        }

        if owners.synced().iter().any(|o| o.address == from_address) {
            owner_of.insert(resource.tag().clone());
        }
    }

    if writer_of.is_empty()
        && owner_of.is_empty()
        && external_writer_of.is_empty()
        && external_owner_of.is_empty()
    {
        migration_ui.stop();

        println!("No permissions to clone.");
        return Ok(());
    }

    migration_ui.stop();

    let mut writers_resource_selectors = writer_of
        .iter()
        .map(|r| dojo_types::naming::compute_selector_from_tag_or_name(r))
        .collect::<Vec<_>>();
    let mut owners_resource_selectors = owner_of
        .iter()
        .map(|r| dojo_types::naming::compute_selector_from_tag_or_name(r))
        .collect::<Vec<_>>();

    writers_resource_selectors.extend(external_writer_of.iter().copied());
    owners_resource_selectors.extend(external_owner_of.iter().copied());

    writer_of.extend(
        external_writer_of
            .iter()
            .map(|r| if r != &WORLD { format!("{:#066x}", r) } else { "World".to_string() }),
    );
    owner_of.extend(
        external_owner_of
            .iter()
            .map(|r| if r != &WORLD { format!("{:#066x}", r) } else { "World".to_string() }),
    );

    // Sort the tags to have a deterministic output.
    let mut writer_of = writer_of.into_iter().collect::<Vec<_>>();
    writer_of.sort();
    let mut owner_of = owner_of.into_iter().collect::<Vec<_>>();
    owner_of.sort();

    let writers_of_tags = writer_of.into_iter().collect::<Vec<_>>().join(", ");
    let owners_of_tags = owner_of.into_iter().collect::<Vec<_>>().join(", ");

    let writers_txt = if writers_of_tags.is_empty() {
        "".to_string()
    } else {
        format!("\n    writers: {}", writers_of_tags)
    };

    let owners_txt = if owners_of_tags.is_empty() {
        "".to_string()
    } else {
        format!("\n    owners: {}", owners_of_tags)
    };

    println!(
        "Confirm the following permissions to be cloned from {} to {}\n{}{}",
        from_tag_or_address.bright_blue(),
        to_tag_or_address.bright_blue(),
        writers_txt.bright_green(),
        owners_txt.bright_yellow(),
    );

    let confirm = utils::prompt_confirm("\nContinue?")?;
    if !confirm {
        return Ok(());
    }

    let world = WorldContract::new(world_diff.world_info.address, &account);
    let mut invoker = Invoker::new(&account, options.transaction.clone().try_into()?);

    for w in writers_resource_selectors.iter() {
        invoker.add_call(world.grant_writer_getcall(w, &ContractAddress(to_address)));
    }

    for o in owners_resource_selectors.iter() {
        invoker.add_call(world.grant_owner_getcall(o, &ContractAddress(to_address)));
    }

    if revoke_from {
        println!(
            "{}",
            format!("\n!Permissions from {} will be revoked!", from_tag_or_address).bright_red()
        );
        if !utils::prompt_confirm("\nContinue?")? {
            return Ok(());
        }

        for w in writers_resource_selectors.iter() {
            invoker.add_call(world.revoke_writer_getcall(w, &ContractAddress(from_address)));
        }

        for o in owners_resource_selectors.iter() {
            invoker.add_call(world.revoke_owner_getcall(o, &ContractAddress(from_address)));
        }
    }

    let res = invoker.multicall().await?;
    println!("{}", res);

    Ok(())
}

/// Resolves the address or tag to an address.
fn resolve_address_or_tag(address_or_tag: &str, world_diff: &WorldDiff) -> Result<Felt> {
    if address_or_tag.starts_with("0x") {
        Felt::from_str(address_or_tag)
            .map_err(|_| anyhow!("Invalid contract address: {}", address_or_tag))
    } else {
        world_diff
            .get_contract_address_from_tag(address_or_tag)
            .ok_or_else(|| anyhow!("Contract {} not found.", address_or_tag))
    }
}

/// Lists the permissions of a resource.
async fn list_permissions(
    resource: Option<String>,
    show_address: bool,
    starknet: StarknetOptions,
    world: WorldOptions,
    scarb_metadata: &Metadata,
) -> Result<()> {
    let mut migration_ui = MigrationUi::new_with_frames(
        "Gathering permissions from the world...",
        vec!["🌍", "🔍", "📜"],
    );

    let (world_diff, _, _) =
        utils::get_world_diff_and_provider(starknet, world, scarb_metadata).await?;

    // Sort resources by tag for deterministic output.
    let mut resources = world_diff.resources.values().collect::<Vec<_>>();
    resources.sort_by_key(|r| r.tag().clone());

    migration_ui.stop();

    let mut world_writers = world_diff
        .external_writers
        .get(&WORLD)
        .map(|writers| writers.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    let mut world_owners = world_diff
        .external_owners
        .get(&WORLD)
        .map(|owners| owners.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    // Sort the tags to have a deterministic output.
    world_writers.sort();
    world_owners.sort();

    println!("{}", "World".bright_red());
    if !world_writers.is_empty() {
        println!(
            "writers: {}",
            world_writers.iter().map(|w| format!("{:#066x}", w)).collect::<Vec<_>>().join(", ")
        );
    }

    if !world_owners.is_empty() {
        println!(
            "owners: {}",
            world_owners.iter().map(|o| format!("{:#066x}", o)).collect::<Vec<_>>().join(", ")
        );
    }

    println!();

    if let Some(resource) = resource {
        let selector = dojo_types::naming::compute_selector_from_tag_or_name(&resource);
        resources.retain(|r| r.dojo_selector() == selector);

        if resources.is_empty() {
            anyhow::bail!("Resource {} not found.", resource.bright_blue());
        }
    }

    if resources.is_empty() {
        println!("No resource found.");
        return Ok(());
    }

    let mut has_printed_at_least_one = false;

    for resource in resources.iter() {
        let selector = resource.dojo_selector();
        let writers = world_diff.get_writers(selector);
        let owners = world_diff.get_owners(selector);

        if writers.is_empty() && owners.is_empty() {
            continue;
        }

        has_printed_at_least_one = true;

        println!("{}", resource.tag().bright_blue());

        if !writers.is_empty() {
            println!("writers: ");
            print_diff_permissions(&writers, show_address);
        }

        if !owners.is_empty() {
            println!("owners: ");
            print_diff_permissions(&owners, show_address);
        }

        println!();
    }

    if resources.len() == 1 && !has_printed_at_least_one {
        println!("No permission found.");
    }

    Ok(())
}

/// Pretty prints the permissions of a resource.
fn print_diff_permissions(diff: &DiffPermissions, show_address: bool) {
    if !diff.only_local().is_empty() {
        println!(
            "    local: {}",
            diff.only_local()
                .iter()
                .map(|w| format!(
                    "{} {}",
                    w.tag.clone().unwrap_or("external".to_string()),
                    if show_address {
                        format!("({:#066x})", w.address).bright_black()
                    } else {
                        "".to_string().bright_black()
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if !diff.only_remote().is_empty() {
        println!(
            "    remote: {}",
            diff.only_remote()
                .iter()
                .map(|w| format!(
                    "{} {}",
                    w.tag.clone().unwrap_or("external".to_string()),
                    if show_address {
                        format!("({:#066x})", w.address).bright_black()
                    } else {
                        "".to_string().bright_black()
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if !diff.synced().is_empty() {
        println!(
            "    synced: {}",
            diff.synced()
                .iter()
                .map(|w| format!(
                    "{} {}",
                    w.tag.clone().unwrap_or("external".to_string()),
                    if show_address {
                        format!("({:#066x})", w.address).bright_black()
                    } else {
                        "".to_string().bright_black()
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

/// Updates the owners permissions.
async fn update_owners(
    contracts: &HashMap<String, ContractInfo>,
    options: &CommonAuthOptions,
    profile_config: &ProfileConfig,
    pairs: Vec<PermissionPair>,
    do_grant: bool,
) -> Result<()> {
    let selectors_addresses = pairs
        .iter()
        .map(|p| p.to_selector_and_address(contracts))
        .collect::<Result<Vec<(Felt, Felt)>>>()?;

    let world = get_world_contract(contracts, options, profile_config).await?;

    let mut invoker = Invoker::new(&world.account, options.transaction.clone().try_into()?);
    for (selector, address) in selectors_addresses {
        let call = if do_grant {
            trace!(
                selector = format!("{:#066x}", selector),
                address = format!("{:#066x}", address),
                "Grant owner call."
            );
            world.grant_owner_getcall(&selector, &ContractAddress(address))
        } else {
            trace!(
                selector = format!("{:#066x}", selector),
                address = format!("{:#066x}", address),
                "Revoke owner call."
            );
            world.revoke_owner_getcall(&selector, &ContractAddress(address))
        };

        invoker.add_call(call);
    }

    let res = invoker.multicall().await?;
    println!("{}", res);

    Ok(())
}

/// Updates the writers permissions.
async fn update_writers(
    contracts: &HashMap<String, ContractInfo>,
    options: &CommonAuthOptions,
    profile_config: &ProfileConfig,
    pairs: Vec<PermissionPair>,
    do_grant: bool,
) -> Result<()> {
    let selectors_addresses = pairs
        .iter()
        .map(|p| p.to_selector_and_address(contracts))
        .collect::<Result<Vec<(Felt, Felt)>>>()?;

    let world = get_world_contract(contracts, options, profile_config).await?;

    let mut invoker = Invoker::new(&world.account, options.transaction.clone().try_into()?);
    for (selector, address) in selectors_addresses {
        let call = if do_grant {
            trace!(
                selector = format!("{:#066x}", selector),
                address = format!("{:#066x}", address),
                "Grant writer call."
            );
            world.grant_writer_getcall(&selector, &ContractAddress(address))
        } else {
            trace!(
                selector = format!("{:#066x}", selector),
                address = format!("{:#066x}", address),
                "Revoke writer call."
            );
            world.revoke_writer_getcall(&selector, &ContractAddress(address))
        };

        invoker.add_call(call);
    }

    let res = invoker.multicall().await?;
    println!("{}", res);

    Ok(())
}

/// Gets the world contract from the contracts map and initializes a world contract instance
/// from the environment.
async fn get_world_contract(
    contracts: &HashMap<String, ContractInfo>,
    options: &CommonAuthOptions,
    profile_config: &ProfileConfig,
) -> Result<WorldContract<SozoAccount<JsonRpcClient<HttpTransport>>>> {
    let env = profile_config.env.as_ref();
    let (provider, _) = options.starknet.provider(env)?;
    let account = options.account.account(provider, env, &options.starknet, contracts).await?;
    let world_address = contracts
        .get("world")
        .ok_or_else(|| anyhow!("World contract not found in the manifest."))?
        .address;

    let world = WorldContract::new(world_address, account);

    Ok(world)
}

#[derive(Debug, Clone, PartialEq)]
pub struct PermissionPair {
    pub resource_tag: String,
    pub grantee_tag_or_address: String,
}

impl PermissionPair {
    /// Returns the selector and the contract address from the permission pair.
    ///
    /// If the grantee tag is not found in the contracts (from the manifest or from the diff), an
    /// error is returned as we're expecting the resource to be resolved locally.
    pub fn to_selector_and_address(
        &self,
        contracts: &HashMap<String, ContractInfo>,
    ) -> Result<(Felt, Felt)> {
        let selector = if self.resource_tag == "world" {
            WORLD
        } else if self.resource_tag.starts_with("0x") {
            Felt::from_str(&self.resource_tag)
                .map_err(|_| anyhow!("Invalid resource selector: {}", self.resource_tag))?
        } else {
            dojo_types::naming::compute_selector_from_tag_or_name(&self.resource_tag)
        };

        let contract_address = if self.grantee_tag_or_address.starts_with("0x") {
            Felt::from_str(&self.grantee_tag_or_address)
                .map_err(|_| anyhow!("Invalid contract address: {}", self.grantee_tag_or_address))?
        } else {
            contracts
                .get(&self.grantee_tag_or_address)
                .ok_or_else(|| {
                    anyhow!("Contract {} not found in the manifest.", self.grantee_tag_or_address)
                })?
                .address
        };

        Ok((selector, contract_address))
    }
}

impl FromStr for PermissionPair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();

        let (resource_tag, grantee_tag_or_address) = match parts.as_slice() {
            [resource_tag, grantee_tag_or_address] => {
                (resource_tag.to_string(), grantee_tag_or_address.to_string())
            }
            _ => anyhow::bail!(
                "Resource and contract are expected to be comma separated: `sozo auth grant \
                 writer resource_tag,contract_tag_or_address`"
            ),
        };

        Ok(PermissionPair { resource_tag, grantee_tag_or_address })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_permission_pair_from_str() {
        let pair = PermissionPair::from_str("moves,actions").unwrap();
        assert_eq!(pair.resource_tag, "moves");
        assert_eq!(pair.grantee_tag_or_address, "actions");

        let pair = PermissionPair::from_str("moves,0x123").unwrap();
        assert_eq!(pair.resource_tag, "moves");
        assert_eq!(pair.grantee_tag_or_address, "0x123");

        assert!(PermissionPair::from_str("moves").is_err());
        assert!(PermissionPair::from_str("moves,actions,extra").is_err());
    }

    #[test]
    fn test_permission_pair_to_selector_and_address() {
        let mut contracts = HashMap::new();
        contracts.insert(
            "actions".to_string(),
            ContractInfo {
                tag_or_name: "actions".to_string(),
                address: Felt::from_str("0x456").unwrap(),
                entrypoints: vec![],
            },
        );

        let pair = PermissionPair {
            resource_tag: "moves".to_string(),
            grantee_tag_or_address: "actions".to_string(),
        };

        let (selector, address) = pair.to_selector_and_address(&contracts).unwrap();

        assert_eq!(selector, dojo_types::naming::compute_selector_from_tag_or_name("moves"));
        assert_eq!(address, Felt::from_str("0x456").unwrap());

        let pair = PermissionPair {
            resource_tag: "moves".to_string(),
            grantee_tag_or_address: "0x123".to_string(),
        };
        let (selector, address) = pair.to_selector_and_address(&contracts).unwrap();
        assert_eq!(selector, dojo_types::naming::compute_selector_from_tag_or_name("moves"));
        assert_eq!(address, Felt::from_str("0x123").unwrap());

        let pair = PermissionPair {
            resource_tag: "moves".to_string(),
            grantee_tag_or_address: "nonexistent".to_string(),
        };
        assert!(pair.to_selector_and_address(&contracts).is_err());

        let pair = PermissionPair {
            resource_tag: "moves".to_string(),
            grantee_tag_or_address: "0xinvalid".to_string(),
        };
        assert!(pair.to_selector_and_address(&contracts).is_err());

        let pair = PermissionPair {
            resource_tag: "world".to_string(),
            grantee_tag_or_address: "0x123".to_string(),
        };
        let (selector, address) = pair.to_selector_and_address(&contracts).unwrap();
        assert_eq!(selector, WORLD);
        assert_eq!(address, Felt::from_str("0x123").unwrap());

        let pair = PermissionPair {
            resource_tag: "0x123".to_string(),
            grantee_tag_or_address: "0x456".to_string(),
        };
        let (selector, address) = pair.to_selector_and_address(&contracts).unwrap();
        assert_eq!(selector, Felt::from_str("0x123").unwrap());
        assert_eq!(address, Felt::from_str("0x456").unwrap());
    }
}
