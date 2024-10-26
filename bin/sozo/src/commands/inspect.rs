use anyhow::Result;
use clap::Args;
use colored::*;
use dojo_types::naming;
use dojo_world::config::ProfileConfig;
use dojo_world::diff::{ResourceDiff, WorldDiff, WorldStatus};
use dojo_world::local::WorldLocal;
use dojo_world::remote::WorldRemote;
use dojo_world::{utils as world_utils, ResourceType};
use scarb::core::Config;
use sozo_ops::migrate::deployer;
use sozo_ops::scarb_extensions::WorkspaceExt;
use starknet::core::types::Felt;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tracing::trace;

use super::options::starknet::StarknetOptions;
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

            if let Some(resource) = resource {
                inspect_resource(&resource, &world_diff, world_address);
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
    #[tabled(rename = "Status")]
    status: ResourceStatus,
    #[tabled(rename = "Contract Address")]
    address: String,
    #[tabled(rename = "Class Hash")]
    current_class_hash: String,
}

#[derive(Debug, Tabled)]
struct ResourceDetailInspect {
    #[tabled(rename = "Name or Tag")]
    name_or_tag: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
    #[tabled(rename = "Contract Address")]
    address: String,
    #[tabled(rename = "Class Hash")]
    class_hash: String,
}

#[derive(Debug, Tabled)]
struct GranteeDisplay {
    name: String,
    selector: String,
}

/// Inspects a resource.
fn inspect_resource(resource_name_or_tag: &str, world_diff: &WorldDiff, world_address: Felt) {
    let resource_diff = world_diff.resource_diff_from_name_or_tag(resource_name_or_tag);

    if resource_diff.is_none() {
        println!("Resource not found locally.");
        return;
    }

    let resource_diff = resource_diff.unwrap();

    let status = match resource_diff {
        ResourceDiff::Created(_) => ResourceStatus::Created,
        ResourceDiff::Updated(_, _) => ResourceStatus::Updated,
        ResourceDiff::Synced(_) => ResourceStatus::Synced,
    };

    let mut selector = Felt::ZERO;
    if !naming::is_valid_tag(resource_name_or_tag) {
        let r = ResourceNameInspect { name: resource_name_or_tag.to_string(), status };

        selector = naming::compute_bytearray_hash(resource_name_or_tag);

        print_table(&[r], "");
    } else {
        let r = resource_diff_display(resource_diff, world_address);

        selector = naming::compute_selector_from_tag(resource_name_or_tag);

        print_table(&[r], "");
    }

    let remote_writers = world_diff.get_remote_writers();
    let remote_owners = world_diff.get_remote_owners();

    let remote_writers_resource = remote_writers.get(&selector);

    let mut writers_disp = vec![];

    if let Some(writers) = remote_writers_resource {
        for w_selector in writers {
            if let Some(r) = world_diff.resource_diff_from_name_or_tag(resource_name_or_tag) {
                match r {
                    ResourceDiff::Created(local) => {
                        writers_disp.push(GranteeDisplay {
                            name: local.name(),
                            selector: format!("{:#066x}", w_selector),
                        });
                    }
                    ResourceDiff::Updated(_, remote) => {
                        writers_disp.push(GranteeDisplay {
                            name: naming::get_tag(&remote.namespace(), &remote.name()),
                            selector: format!("{:#066x}", w_selector),
                        });
                    }
                    ResourceDiff::Synced(remote) => {
                        writers_disp.push(GranteeDisplay {
                            name: naming::get_tag(&remote.namespace(), &remote.name()),
                            selector: format!("{:#066x}", w_selector),
                        });
                    }
                }
            }

            writers_disp.push(GranteeDisplay {
                name: w_selector.to_string(),
                selector: format!("{:#066x}", w_selector),
            });
        }
    }
}

/// Inspects the whole world.
fn inspect_world(world_diff: &WorldDiff, world_address: Felt) {
    println!("");

    let mut disp_namespaces = vec![];

    for ns_selector in &world_diff.namespaces {
        let rns = world_diff.resources.get(ns_selector).unwrap();
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

    print_table(&disp_namespaces, "> Namespaces");

    let mut contracts_disp = vec![];

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

    let mut models_disp = vec![];
    let mut events_disp = vec![];

    for (_selector, resource) in &world_diff.resources {
        match resource.resource_type() {
            ResourceType::Contract => {
                contracts_disp.push(resource_diff_display(resource, world_address))
            }
            ResourceType::Model => models_disp.push(resource_diff_display(resource, world_address)),
            ResourceType::Event => events_disp.push(resource_diff_display(resource, world_address)),
            _ => {}
        }
    }

    if !contracts_disp.is_empty() {
        contracts_disp.sort_by_key(|m| m.name.clone());
    }

    // Keep world at the top.
    contracts_disp.insert(0, world);
    print_table(&contracts_disp, "> Contracts");

    if !models_disp.is_empty() {
        models_disp.sort_by_key(|m| m.name.clone());

        print_table(&models_disp, "> Models");
    }

    if !events_disp.is_empty() {
        events_disp.sort_by_key(|m| m.name.clone());

        print_table(&events_disp, "> Events");
    }
}

/// Displays the resource diff with the address and class hash.
fn resource_diff_display(
    resource: &ResourceDiff,
    world_address: Felt,
) -> ResourceWithAddressInspect {
    let (name, address, class_hash, status) = match resource {
        ResourceDiff::Created(local) => (
            local.tag(),
            world_utils::compute_dojo_contract_address(
                local.dojo_selector(),
                local.class_hash(),
                world_address,
            ),
            local.class_hash(),
            ResourceStatus::Created,
        ),
        ResourceDiff::Updated(local, remote) => {
            (local.tag(), remote.address(), local.class_hash(), ResourceStatus::Updated)
        }
        ResourceDiff::Synced(remote) => {
            (remote.tag(), remote.address(), remote.current_class_hash(), ResourceStatus::Synced)
        }
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

    println!("{title}");
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
