use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use colored::*;
use dojo_types::naming;
use dojo_world::config::{Environment, ProfileConfig};
use dojo_world::contracts::{WorldContract, WorldContractReader};
use dojo_world::diff::{ResourceDiff, WorldDiff, WorldStatus};
use dojo_world::local::{ContractLocal, ResourceLocal, WorldLocal};
use dojo_world::remote::WorldRemote;
use dojo_world::utils as world_utils;
use katana_rpc_api::starknet::RPC_SPEC_VERSION;
use scarb::core::{Config, Workspace};
use sozo_ops::migrate::{self, deployer, Migration};
use sozo_ops::scarb_extensions::WorkspaceExt;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, Felt, StarknetError};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_contract_address, parse_cairo_short_string,
};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tracing::trace;

use super::options::account::{AccountOptions, SozoAccount};
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct InspectArgs {
    #[arg(help = "The tag of the resource to inspect. If not provided, a world summary will be \
                  displayed.")]
    resource: Option<String>,

    #[command(flatten)]
    world: WorldOptions,

    #[command(flatten)]
    starknet: StarknetOptions,
}

impl InspectArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let target_dir_profile = ws.target_dir_profile();

        let (_profile_name, profile_config) = utils::load_profile_config(config)?;

        let InspectArgs { world, starknet, resource } = self;

        let world_local = WorldLocal::from_directory(
            target_dir_profile.to_string(),
            profile_config.namespace.clone(),
        )?;

        let world_address = get_world_address(&profile_config, &world, &world_local)?;

        config.tokio_handle().block_on(async {
            let env = profile_config.env.as_ref();

            let provider = starknet.provider(env)?;

            let world_diff = if deployer::is_deployed(world_address, &provider).await? {
                let world_remote = WorldRemote::from_events(world_address, &provider).await?;

                WorldDiff::new(world_local, world_remote)
            } else {
                WorldDiff::from_local(world_local)
            };

            if let Some(_resource) = resource {
                // inspect_resource(world_diff, resource)?;
                // TODO: Show the different permissions, transaction hashes etc...
            } else {
                inspect_world(&world_diff, world_address);
            }

            Ok(())
        })
    }
}

#[derive(Debug)]
enum ResourceStatus {
    Created,
    Updated,
    Synced,
}

impl std::fmt::Display for ResourceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceStatus::Created => write!(f, "{}", "Created".blue()),
            ResourceStatus::Updated => write!(f, "{}", "Updated".yellow()),
            ResourceStatus::Synced => write!(f, "{}", "Synced".green()),
        }
    }
}

#[derive(Debug, Tabled)]
struct ResourceNameInspect {
    #[tabled(rename = "")]
    name: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
}

#[derive(Debug, Tabled)]
struct ResourceWithAddressInspect {
    #[tabled(rename = "")]
    name: String,
    #[tabled(rename = "Contract Address")]
    address: String,
    #[tabled(rename = "Class Hash")]
    current_class_hash: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
}

/// Inspects the whole world.
fn inspect_world(world_diff: &WorldDiff, world_address: Felt) {
    println!("");

    let mut disp_namespaces = vec![];

    for rns in &world_diff.namespaces {
        match rns {
            ResourceDiff::Created(local) => {
                disp_namespaces.push(ResourceNameInspect {
                    name: local.name(),
                    status: ResourceStatus::Created,
                });
            }
            ResourceDiff::Synced(remote) => {
                disp_namespaces.push(ResourceNameInspect {
                    name: remote.name(),
                    status: ResourceStatus::Synced,
                });
            }
            _ => {}
        }
    }

    print_table(&disp_namespaces, "Namespaces");

    let mut disp_resources = vec![];

    let world = match &world_diff.world_status {
        WorldStatus::NewVersion(class_hash, _, _) => ResourceWithAddressInspect {
            name: "World".to_string(),
            address: format!("{:#066x}", world_address),
            current_class_hash: format!("{:#066x}", class_hash),
            status: ResourceStatus::Created,
        },
        WorldStatus::Synced(class_hash) => ResourceWithAddressInspect {
            name: "World".to_string(),
            address: format!("{:#066x}", world_address),
            current_class_hash: format!("{:#066x}", class_hash),
            status: ResourceStatus::Synced,
        },
    };

    disp_resources.push(world);

    for (namespace, rcs) in &world_diff.contracts {
        for rc in rcs {
            disp_resources.push(resource_diff_display(&rc, world_address, &namespace));
        }
    }

    print_table(&disp_resources, "Contracts");

    disp_resources.clear();

    for (namespace, rms) in &world_diff.models {
        for rm in rms {
            disp_resources.push(resource_diff_display(&rm, world_address, &namespace));
        }
    }

    if !disp_resources.is_empty() {
        print_table(&disp_resources, "Models");
    }

    disp_resources.clear();

    for (namespace, revs) in &world_diff.events {
        for re in revs {
            disp_resources.push(resource_diff_display(&re, world_address, &namespace));
        }
    }

    if !disp_resources.is_empty() {
        print_table(&disp_resources, "Events");
    }
}

/// Displays the resource diff with the address and class hash.
fn resource_diff_display(
    resource: &ResourceDiff,
    world_address: Felt,
    namespace: &str,
) -> ResourceWithAddressInspect {
    let (name, address, class_hash, status) = match resource {
        ResourceDiff::Created(local) => (
            naming::get_tag(namespace, &local.name()),
            world_utils::compute_dojo_contract_address(
                local.dojo_selector(namespace),
                local.class_hash(),
                world_address,
            ),
            local.class_hash(),
            ResourceStatus::Created,
        ),
        ResourceDiff::Updated(local, remote) => (
            naming::get_tag(namespace, &local.name()),
            remote.address(),
            local.class_hash(),
            ResourceStatus::Updated,
        ),
        ResourceDiff::Synced(remote) => (
            naming::get_tag(namespace, &remote.name()),
            remote.address(),
            remote.current_class_hash(),
            ResourceStatus::Synced,
        ),
    };

    ResourceWithAddressInspect {
        name,
        address: format!("{:#066x}", address),
        current_class_hash: format!("{:#066x}", class_hash),
        status,
    }
}

/// Prints a table.
fn print_table<T>(data: T, title: &str)
where
    T: IntoIterator,
    <T as IntoIterator>::Item: Tabled,
{
    let mut table = Table::new(data);
    table.with(Style::modern());

    println!("** {title} **");
    println!("{table}\n");
}

/// Computes the world address based on the provided options.
fn get_world_address(
    profile_config: &ProfileConfig,
    world: &WorldOptions,
    world_local: &WorldLocal,
) -> Result<Felt> {
    let env = profile_config.env.as_ref();

    let deterministic_world_address =
        world_local.compute_world_address(&profile_config.world.seed)?;

    if let Some(wa) = world.address(env)? { Ok(wa) } else { Ok(deterministic_world_address) }
}
