use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use cainome::cairo_serde::ContractAddress;
use clap::{Args, Subcommand};
use colored::Colorize;
use dojo_utils::{Invoker, TxnConfig};
use dojo_world::config::{calldata_decoder, ProfileConfig};
use dojo_world::contracts::{ContractInfo, WorldContract};
use dojo_world::diff::DiffPermissions;
use scarb::core::{Config, Workspace};
use sozo_ops::migration_ui::MigrationUi;
use sozo_ops::resource_descriptor::ResourceDescriptor;
use sozo_scarbext::WorkspaceExt;
use sozo_walnut::WalnutDebugger;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{Call, Felt};
use starknet::core::utils as snutils;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
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
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let profile_config = ws.load_profile_config()?;

        config.tokio_handle().block_on(async {
            match self.command {
                AuthCommand::Grant { kind, common, .. } => {
                    let contracts = utils::contracts_from_manifest_or_diff(
                        common.account.clone(),
                        common.starknet.clone(),
                        common.world.clone(),
                        &ws,
                        false,
                    )
                    .await?;

                    let do_grant = true;

                    match kind {
                        AuthKind::Writer { pairs } => {
                            let is_writer = true;
                            update_permissions(
                                &contracts,
                                &common,
                                &profile_config,
                                pairs,
                                is_writer,
                                do_grant,
                            )
                            .await?;
                        }
                        AuthKind::Owner { pairs } => {
                            let is_writer = false;
                            update_permissions(
                                &contracts,
                                &common,
                                &profile_config,
                                pairs,
                                is_writer,
                                do_grant,
                            )
                            .await?;
                        }
                    }
                }
                AuthCommand::Revoke { kind, common, .. } => {
                    let contracts = utils::contracts_from_manifest_or_diff(
                        common.account.clone(),
                        common.starknet.clone(),
                        common.world.clone(),
                        &ws,
                        false,
                    )
                    .await?;

                    let do_grant = false;

                    match kind {
                        AuthKind::Writer { pairs } => {
                            let is_writer = true;
                            update_permissions(
                                &contracts,
                                &common,
                                &profile_config,
                                pairs,
                                is_writer,
                                do_grant,
                            )
                            .await?;
                        }
                        AuthKind::Owner { pairs } => {
                            let is_writer = false;
                            update_permissions(
                                &contracts,
                                &common,
                                &profile_config,
                                pairs,
                                is_writer,
                                do_grant,
                            )
                            .await?;
                        }
                    }
                }
                AuthCommand::List { resource, show_address, starknet, world } => {
                    list_permissions(resource, show_address, starknet, world, &ws).await?;
                }
            };

            Ok(())
        })
    }
}

/// Lists the permissions of a resource.
async fn list_permissions(
    resource: Option<String>,
    show_address: bool,
    starknet: StarknetOptions,
    world: WorldOptions,
    ws: &Workspace<'_>,
) -> Result<()> {
    let mut migration_ui = MigrationUi::new_with_frames(
        "Gathering permissions from the world...",
        vec!["🌍", "🔍", "📜"],
    );

    let (world_diff, _, _) = utils::get_world_diff_and_provider(starknet, world, ws).await?;

    // Sort resources by tag for deterministic output.
    let mut resources = world_diff.resources.values().collect::<Vec<_>>();
    resources.sort_by_key(|r| r.tag().clone());

    migration_ui.stop();

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

/// Updates the permissions of a resource for a contract.
async fn update_permissions(
    contracts: &HashMap<String, ContractInfo>,
    options: &CommonAuthOptions,
    profile_config: &ProfileConfig,
    pairs: Vec<PermissionPair>,
    is_writer: bool,
    do_grant: bool,
) -> Result<()> {
    let selectors_addresses = pairs
        .iter()
        .map(|p| p.to_selector_and_address(&contracts))
        .collect::<Result<Vec<(Felt, Felt)>>>()?;

    let world = get_world_contract(contracts, options, profile_config).await?;

    let mut invoker = Invoker::new(&world.account, options.transaction.clone().try_into()?);
    for (selector, address) in selectors_addresses {
        let call = if is_writer {
            if do_grant {
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
            }
        } else if do_grant {
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
        let selector = dojo_types::naming::compute_selector_from_tag_or_name(&self.resource_tag);

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
                tag: "actions".to_string(),
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
    }
}
