use anyhow::Result;
use clap::Args;
use colored::*;
use dojo_types::naming;
use dojo_world::ResourceType;
use dojo_world::diff::{ResourceDiff, WorldDiff, WorldStatus};
use dojo_world::local::ExternalContractLocal;
use scarb_metadata::Metadata;
use serde::Serialize;
use sozo_ui::SozoUi;
use tabled::settings::object::Cell;
use tabled::settings::{Color, Style};
use tabled::{Table, Tabled};
use tracing::trace;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct InspectArgs {
    #[arg(help = "The tag of the resource or the external contract instance name to inspect. If \
                  not provided, a world summary will be displayed.")]
    element: Option<String>,

    #[arg(long, help = "Output in JSON format")]
    json: bool,

    #[command(flatten)]
    world: WorldOptions,

    #[command(flatten)]
    starknet: StarknetOptions,
}

impl InspectArgs {
    pub async fn run(self, scarb_metadata: &Metadata, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let InspectArgs { world, starknet, element, json } = self;

        let (world_diff, _, _) =
            utils::get_world_diff_and_provider(starknet.clone(), world, scarb_metadata, ui).await?;

        if let Some(element) = element {
            inspect_element(&element, &world_diff, json)?;
        } else {
            inspect_world(&world_diff, json);
        }

        Ok(())
    }
}

#[derive(Debug, Serialize)]
enum ResourceStatus {
    Created,
    Updated,
    Synced,
    DirtyLocalPerms,
    MigrationSkipped,
}

impl std::fmt::Display for ResourceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceStatus::Created => write!(f, "{}", "Created".blue()),
            ResourceStatus::Updated => write!(f, "{}", "Updated".yellow()),
            ResourceStatus::Synced => write!(f, "{}", "Synced".green()),
            ResourceStatus::DirtyLocalPerms => write!(f, "{}", "Dirty local perms".yellow()),
            ResourceStatus::MigrationSkipped => write!(f, "{}", "Migration skipped".bright_black()),
        }
    }
}

#[derive(Debug, Tabled, Serialize)]
enum ResourceInspect {
    Namespace(NamespaceInspect),
    Contract(ContractInspect),
    ExternalContract(ExternalContractInspect),
    Model(ModelInspect),
    Event(EventInspect),
    Library(LibraryInspect),
}

#[derive(Debug, Tabled, Serialize)]
struct NamespaceInspect {
    #[tabled(rename = "Namespaces")]
    name: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
    #[tabled(rename = "Dojo Selector")]
    selector: String,
}

#[derive(Debug, Tabled, Serialize)]
struct WorldInspect {
    #[tabled(rename = "World")]
    status: ResourceStatus,
    #[tabled(rename = "Contract Address")]
    address: String,
    #[tabled(rename = "Class Hash")]
    current_class_hash: String,
}

#[derive(Debug, Tabled, Serialize)]
struct ContractInspect {
    #[tabled(rename = "Contracts")]
    tag: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
    #[tabled(rename = "Is Initialized")]
    is_initialized: bool,
    #[tabled(rename = "Dojo Selector")]
    selector: String,
    #[tabled(rename = "Contract Address")]
    address: String,
    #[tabled(skip)]
    current_class_hash: String,
}

#[derive(Debug, Tabled, Serialize)]
struct LibraryInspect {
    #[tabled(rename = "Libraries")]
    tag: String,
    #[tabled(rename = "Version")]
    version: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
    #[tabled(rename = "Dojo Selector")]
    selector: String,
    #[tabled(rename = "Class Hash")]
    current_class_hash: String,
}

#[derive(Debug, Tabled, Serialize)]
struct ModelInspect {
    #[tabled(rename = "Models")]
    tag: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
    #[tabled(rename = "Dojo Selector")]
    selector: String,
}

#[derive(Debug, Tabled, Serialize)]
struct EventInspect {
    #[tabled(rename = "Events")]
    tag: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
    #[tabled(rename = "Dojo Selector")]
    selector: String,
}

#[derive(Debug, Tabled, Serialize)]
struct ExternalContractInspect {
    #[tabled(rename = "External Contract")]
    contract_name: String,
    #[tabled(rename = "Instance Name")]
    tag: String,
    #[tabled(skip)]
    class_hash: String,
    #[tabled(rename = "Status")]
    status: ResourceStatus,
    #[tabled(rename = "Dojo Selector")]
    selector: String,
    #[tabled(rename = "Contract Address")]
    address: String,
}

#[derive(Debug, Tabled)]
enum GranteeSource {
    #[tabled(rename = "Local")]
    Local,
    #[tabled(rename = "Remote")]
    Remote,
    #[tabled(rename = "Synced")]
    Synced,
}

impl std::fmt::Display for GranteeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GranteeSource::Local => write!(f, "{}", "Local".blue()),
            GranteeSource::Remote => write!(f, "{}", "Remote".bright_black()),
            GranteeSource::Synced => write!(f, "{}", "Synced".green()),
        }
    }
}

#[derive(Debug, Tabled)]
struct GranteeDisplay {
    #[tabled(rename = "Tag")]
    tag: String,
    #[tabled(rename = "Contract Address")]
    address: String,
    #[tabled(rename = "Source")]
    source: GranteeSource,
}

// JSON output structures
#[derive(Debug, Serialize)]
struct JsonWorldInspect {
    world: JsonWorldInfo,
    namespaces: Vec<JsonNamespaceInfo>,
    contracts: Vec<JsonContractInfo>,
    libraries: Vec<JsonLibraryInfo>,
    models: Vec<JsonModelInfo>,
    events: Vec<JsonEventInfo>,
    external_contracts: Vec<JsonExternalContractInfo>,
}

#[derive(Debug, Serialize)]
struct JsonWorldInfo {
    status: String,
    address: String,
    class_hash: String,
}

#[derive(Debug, Serialize)]
struct JsonNamespaceInfo {
    name: String,
    status: String,
    selector: String,
}

#[derive(Debug, Serialize)]
struct JsonContractInfo {
    tag: String,
    status: String,
    is_initialized: bool,
    selector: String,
    address: String,
    class_hash: String,
}

#[derive(Debug, Serialize)]
struct JsonLibraryInfo {
    tag: String,
    version: String,
    status: String,
    selector: String,
    class_hash: String,
}

#[derive(Debug, Serialize)]
struct JsonModelInfo {
    tag: String,
    status: String,
    selector: String,
}

#[derive(Debug, Serialize)]
struct JsonEventInfo {
    tag: String,
    status: String,
    selector: String,
}

#[derive(Debug, Serialize)]
struct JsonExternalContractInfo {
    contract_name: String,
    instance_name: String,
    address: String,
    class_hash: String,
    status: String,
    salt: String,
    constructor_calldata: Vec<String>,
}

#[derive(Debug, Serialize)]
struct JsonResourceInspect {
    resource: serde_json::Value,
    writers: Vec<JsonGranteeInfo>,
    owners: Vec<JsonGranteeInfo>,
}

#[derive(Debug, Serialize)]
struct JsonGranteeInfo {
    tag: String,
    address: String,
    source: String,
}

fn print_json<T: serde::Serialize>(data: &T) {
    println!("{}", serde_json::to_string_pretty(data).unwrap());
}

/// Inspects a world element (resource or external contract).
fn inspect_element(element_name: &str, world_diff: &WorldDiff, json: bool) -> Result<()> {
    let selector = if naming::is_valid_tag(element_name) {
        naming::compute_selector_from_tag(element_name)
    } else {
        naming::compute_bytearray_hash(element_name)
    };

    if let Some(diff) = world_diff.resources.get(&selector) {
        if json {
            inspect_resource_json(diff, world_diff)
        } else {
            inspect_resource(diff, world_diff)
        }
    } else {
        Err(anyhow::anyhow!("Resource or external contract not found locally."))
    }
}

/// Inspects a resource in JSON format.
fn inspect_resource_json(resource_diff: &ResourceDiff, world_diff: &WorldDiff) -> Result<()> {
    let writers = world_diff.get_writers(resource_diff.dojo_selector());
    let mut writers_json = vec![];

    for pdiff in writers.only_local() {
        writers_json.push(JsonGranteeInfo {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: "Local".to_string(),
        });
    }

    for pdiff in writers.only_remote() {
        writers_json.push(JsonGranteeInfo {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: "Remote".to_string(),
        });
    }

    for pdiff in writers.synced() {
        writers_json.push(JsonGranteeInfo {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: "Synced".to_string(),
        });
    }

    let owners = world_diff.get_owners(resource_diff.dojo_selector());
    let mut owners_json = vec![];

    for pdiff in owners.only_local() {
        owners_json.push(JsonGranteeInfo {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: "Local".to_string(),
        });
    }

    for pdiff in owners.only_remote() {
        owners_json.push(JsonGranteeInfo {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: "Remote".to_string(),
        });
    }

    for pdiff in owners.synced() {
        owners_json.push(JsonGranteeInfo {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: "Synced".to_string(),
        });
    }

    writers_json.sort_by_key(|m| m.tag.clone());
    owners_json.sort_by_key(|m| m.tag.clone());

    // Create the resource JSON data
    let resource_json = match resource_diff.resource_type() {
        ResourceType::Namespace => {
            let status = match resource_diff {
                ResourceDiff::Created(_) => "Created".to_string(),
                ResourceDiff::Synced(_, _) => "Synced".to_string(),
                _ => unreachable!(),
            };
            serde_json::json!({
                "type": "namespace",
                "name": resource_diff.name(),
                "status": status,
                "selector": format!("{:#066x}", resource_diff.dojo_selector())
            })
        }
        ResourceType::Contract => {
            let (is_initialized, contract_address, status) = match resource_diff {
                ResourceDiff::Created(_) => (
                    false,
                    world_diff.get_contract_address(resource_diff.dojo_selector()).unwrap(),
                    "Created".to_string(),
                ),
                ResourceDiff::Updated(_, remote) => (
                    remote.as_contract_or_panic().is_initialized,
                    remote.address(),
                    "Updated".to_string(),
                ),
                ResourceDiff::Synced(_, remote) => (
                    remote.as_contract_or_panic().is_initialized,
                    remote.address(),
                    "Synced".to_string(),
                ),
            };
            serde_json::json!({
                "type": "contract",
                "tag": resource_diff.tag(),
                "status": status,
                "is_initialized": is_initialized,
                "selector": format!("{:#066x}", resource_diff.dojo_selector()),
                "address": format!("{:#066x}", contract_address),
                "class_hash": format!("{:#066x}", resource_diff.current_class_hash())
            })
        }
        ResourceType::Library => {
            let status = match resource_diff {
                ResourceDiff::Created(_) => "Created".to_string(),
                ResourceDiff::Updated(_, _) => "Updated".to_string(),
                ResourceDiff::Synced(_, _) => "Synced".to_string(),
            };
            let version = world_diff
                .profile_config
                .lib_versions
                .as_ref()
                .expect("expected lib_versions")
                .get(&resource_diff.tag())
                .expect("lib_version not found");
            serde_json::json!({
                "type": "library",
                "tag": resource_diff.tag(),
                "version": version.to_string(),
                "status": status,
                "selector": format!("{:#066x}", resource_diff.dojo_selector()),
                "class_hash": format!("{:#066x}", resource_diff.current_class_hash())
            })
        }
        ResourceType::Model => {
            let status = match resource_diff {
                ResourceDiff::Created(_) => "Created".to_string(),
                ResourceDiff::Updated(_, _) => "Updated".to_string(),
                ResourceDiff::Synced(_, _) => "Synced".to_string(),
            };
            serde_json::json!({
                "type": "model",
                "tag": resource_diff.tag(),
                "status": status,
                "selector": format!("{:#066x}", resource_diff.dojo_selector())
            })
        }
        ResourceType::Event => {
            let status = match resource_diff {
                ResourceDiff::Created(_) => "Created".to_string(),
                ResourceDiff::Updated(_, _) => "Updated".to_string(),
                ResourceDiff::Synced(_, _) => "Synced".to_string(),
            };
            serde_json::json!({
                "type": "event",
                "tag": resource_diff.tag(),
                "status": status,
                "selector": format!("{:#066x}", resource_diff.dojo_selector())
            })
        }
        ResourceType::ExternalContract => {
            let (external_contract, contract_address, status) = match resource_diff {
                ResourceDiff::Created(local) => {
                    let local = local.as_external_contract().unwrap();
                    let address = match local {
                        ExternalContractLocal::SozoManaged(l) => l.computed_address,
                        ExternalContractLocal::SelfManaged(l) => l.contract_address,
                    };
                    (local, address, "Created".to_string())
                }
                ResourceDiff::Updated(local, remote) => {
                    let local = local.as_external_contract().unwrap();
                    let remote = remote.as_external_contract_or_panic();
                    (local, remote.common.address, "Updated".to_string())
                }
                ResourceDiff::Synced(local, remote) => {
                    let local = local.as_external_contract().unwrap();
                    let remote = remote.as_external_contract_or_panic();
                    (local, remote.common.address, "Synced".to_string())
                }
            };
            let contract_name = match external_contract {
                ExternalContractLocal::SozoManaged(c) => c.contract_name.clone(),
                ExternalContractLocal::SelfManaged(c) => c.name.clone(),
            };
            serde_json::json!({
                "type": "external_contract",
                "contract_name": contract_name,
                "instance_name": resource_diff.tag(),
                "status": status,
                "selector": format!("{:#066x}", resource_diff.dojo_selector()),
                "address": format!("{:#066x}", contract_address),
                "class_hash": format!("{:#066x}", resource_diff.current_class_hash())
            })
        }
    };

    let json_inspect =
        JsonResourceInspect { resource: resource_json, writers: writers_json, owners: owners_json };

    print_json(&json_inspect);
    Ok(())
}

/// Inspects a resource.
fn inspect_resource(resource_diff: &ResourceDiff, world_diff: &WorldDiff) -> Result<()> {
    let inspect = resource_diff_display(world_diff, resource_diff);
    pretty_print_toml(&toml::to_string_pretty(&inspect).unwrap());

    let writers = world_diff.get_writers(resource_diff.dojo_selector());
    let mut writers_disp = vec![];

    for pdiff in writers.only_local() {
        writers_disp.push(GranteeDisplay {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: GranteeSource::Local,
        });
    }

    for pdiff in writers.only_remote() {
        writers_disp.push(GranteeDisplay {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: GranteeSource::Remote,
        });
    }

    for pdiff in writers.synced() {
        writers_disp.push(GranteeDisplay {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: GranteeSource::Synced,
        });
    }

    let owners = world_diff.get_owners(resource_diff.dojo_selector());
    let mut owners_disp = vec![];

    for pdiff in owners.only_local() {
        owners_disp.push(GranteeDisplay {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: GranteeSource::Local,
        });
    }

    for pdiff in owners.only_remote() {
        owners_disp.push(GranteeDisplay {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: GranteeSource::Remote,
        });
    }

    for pdiff in owners.synced() {
        owners_disp.push(GranteeDisplay {
            tag: pdiff.tag.unwrap_or("external".to_string()),
            address: format!("{:#066x}", pdiff.address),
            source: GranteeSource::Synced,
        });
    }

    writers_disp.sort_by_key(|m| m.tag.to_string());
    owners_disp.sort_by_key(|m| m.tag.to_string());

    print_table(&writers_disp, Some(Color::FG_BRIGHT_CYAN), Some("\n> Writers"));
    print_table(&owners_disp, Some(Color::FG_BRIGHT_MAGENTA), Some("\n> Owners"));
    Ok(())
}

/// Inspects the whole world.
fn inspect_world(world_diff: &WorldDiff, json: bool) {
    if json {
        let world_status = match &world_diff.world_info.status {
            WorldStatus::NotDeployed => "Created".to_string(),
            WorldStatus::NewVersion => "Updated".to_string(),
            WorldStatus::Synced => "Synced".to_string(),
        };

        let world_info = JsonWorldInfo {
            status: world_status,
            address: format!("{:#066x}", world_diff.world_info.address),
            class_hash: format!("{:#066x}", world_diff.world_info.class_hash),
        };

        let mut namespaces = vec![];
        let mut contracts = vec![];
        let mut external_contracts = vec![];
        let mut models = vec![];
        let mut events = vec![];
        let mut libraries = vec![];

        for resource in world_diff.resources.values() {
            match resource.resource_type() {
                ResourceType::Namespace => {
                    let status = match resource {
                        ResourceDiff::Created(_) => "Created".to_string(),
                        ResourceDiff::Updated(_, _) => "Updated".to_string(),
                        ResourceDiff::Synced(_, _) => "Synced".to_string(),
                    };
                    namespaces.push(JsonNamespaceInfo {
                        name: resource.name(),
                        status,
                        selector: format!("{:#066x}", resource.dojo_selector()),
                    });
                }
                ResourceType::Contract => {
                    let (is_initialized, contract_address, status) = match resource {
                        ResourceDiff::Created(_) => (
                            false,
                            world_diff.get_contract_address(resource.dojo_selector()).unwrap(),
                            "Created".to_string(),
                        ),
                        ResourceDiff::Updated(_, remote) => (
                            remote.as_contract_or_panic().is_initialized,
                            remote.address(),
                            "Updated".to_string(),
                        ),
                        ResourceDiff::Synced(_, remote) => (
                            remote.as_contract_or_panic().is_initialized,
                            remote.address(),
                            "Synced".to_string(),
                        ),
                    };
                    contracts.push(JsonContractInfo {
                        tag: resource.tag(),
                        status,
                        is_initialized,
                        selector: format!("{:#066x}", resource.dojo_selector()),
                        address: format!("{:#066x}", contract_address),
                        class_hash: format!("{:#066x}", resource.current_class_hash()),
                    });
                }
                ResourceType::Library => {
                    let status = match resource {
                        ResourceDiff::Created(_) => "Created".to_string(),
                        ResourceDiff::Updated(_, _) => "Updated".to_string(),
                        ResourceDiff::Synced(_, _) => "Synced".to_string(),
                    };
                    let version = world_diff
                        .profile_config
                        .lib_versions
                        .as_ref()
                        .expect("expected lib_versions")
                        .get(&resource.tag())
                        .expect("lib_version not found");
                    libraries.push(JsonLibraryInfo {
                        tag: resource.tag(),
                        version: version.to_string(),
                        status,
                        selector: format!("{:#066x}", resource.dojo_selector()),
                        class_hash: format!("{:#066x}", resource.current_class_hash()),
                    });
                }
                ResourceType::Model => {
                    let status = match resource {
                        ResourceDiff::Created(_) => "Created".to_string(),
                        ResourceDiff::Updated(_, _) => "Updated".to_string(),
                        ResourceDiff::Synced(_, _) => "Synced".to_string(),
                    };
                    models.push(JsonModelInfo {
                        tag: resource.tag(),
                        status,
                        selector: format!("{:#066x}", resource.dojo_selector()),
                    });
                }
                ResourceType::Event => {
                    let status = match resource {
                        ResourceDiff::Created(_) => "Created".to_string(),
                        ResourceDiff::Updated(_, _) => "Updated".to_string(),
                        ResourceDiff::Synced(_, _) => "Synced".to_string(),
                    };
                    events.push(JsonEventInfo {
                        tag: resource.tag(),
                        status,
                        selector: format!("{:#066x}", resource.dojo_selector()),
                    });
                }
                ResourceType::ExternalContract => {
                    let (external_contract, contract_address, status) = match resource {
                        ResourceDiff::Created(local) => {
                            let local = local.as_external_contract().unwrap();
                            let address = match local {
                                ExternalContractLocal::SozoManaged(l) => l.computed_address,
                                ExternalContractLocal::SelfManaged(l) => l.contract_address,
                            };
                            (local, address, "Created".to_string())
                        }
                        ResourceDiff::Updated(local, remote) => {
                            let local = local.as_external_contract().unwrap();
                            let remote = remote.as_external_contract_or_panic();
                            (local, remote.common.address, "Updated".to_string())
                        }
                        ResourceDiff::Synced(local, remote) => {
                            let local = local.as_external_contract().unwrap();
                            let remote = remote.as_external_contract_or_panic();
                            (local, remote.common.address, "Synced".to_string())
                        }
                    };
                    let contract_name = match external_contract {
                        ExternalContractLocal::SozoManaged(c) => c.contract_name.clone(),
                        ExternalContractLocal::SelfManaged(c) => c.name.clone(),
                    };
                    external_contracts.push(JsonExternalContractInfo {
                        contract_name,
                        instance_name: resource.tag(),
                        address: format!("{:#066x}", contract_address),
                        class_hash: format!("{:#066x}", resource.current_class_hash()),
                        status,
                        salt: "".to_string(), // TODO: Add salt if available
                        constructor_calldata: vec![], // TODO: Add constructor calldata if available
                    });
                }
            }
        }

        namespaces.sort_by_key(|n| n.name.clone());
        contracts.sort_by_key(|c| c.tag.clone());
        models.sort_by_key(|m| m.tag.clone());
        events.sort_by_key(|e| e.tag.clone());
        libraries.sort_by_key(|l| l.tag.clone());
        external_contracts.sort_by_key(|c| format!("{}-{}", c.contract_name, c.instance_name));

        let json_world = JsonWorldInspect {
            world: world_info,
            namespaces,
            contracts,
            libraries,
            models,
            events,
            external_contracts,
        };

        print_json(&json_world);
    } else {
        println!();

        let status = match &world_diff.world_info.status {
            WorldStatus::NotDeployed => ResourceStatus::Created,
            WorldStatus::NewVersion => ResourceStatus::Updated,
            WorldStatus::Synced => ResourceStatus::Synced,
        };

        let world = WorldInspect {
            address: format!("{:#066x}", world_diff.world_info.address),
            current_class_hash: format!("{:#066x}", world_diff.world_info.class_hash),
            status,
        };

        print_table(&[world], Some(Color::FG_BRIGHT_BLACK), None);

        let mut namespaces_disp = vec![];
        let mut contracts_disp = vec![];
        let mut external_contracts_disp = vec![];
        let mut models_disp = vec![];
        let mut events_disp = vec![];
        let mut libraries_disp = vec![];

        for resource in world_diff.resources.values() {
            match resource.resource_type() {
                ResourceType::Namespace => match resource_diff_display(world_diff, resource) {
                    ResourceInspect::Namespace(n) => namespaces_disp.push(n),
                    _ => unreachable!(),
                },
                ResourceType::Contract => match resource_diff_display(world_diff, resource) {
                    ResourceInspect::Contract(c) => contracts_disp.push(c),
                    _ => unreachable!(),
                },
                ResourceType::ExternalContract => match resource_diff_display(world_diff, resource)
                {
                    ResourceInspect::ExternalContract(c) => external_contracts_disp.push(c),
                    _ => unreachable!(),
                },
                ResourceType::Model => match resource_diff_display(world_diff, resource) {
                    ResourceInspect::Model(m) => models_disp.push(m),
                    _ => unreachable!(),
                },
                ResourceType::Event => match resource_diff_display(world_diff, resource) {
                    ResourceInspect::Event(e) => events_disp.push(e),
                    _ => unreachable!(),
                },
                ResourceType::Library => match resource_diff_display(world_diff, resource) {
                    ResourceInspect::Library(l) => libraries_disp.push(l),
                    _ => unreachable!(),
                },
            }
        }

        namespaces_disp.sort_by_key(|m| m.name.to_string());
        contracts_disp.sort_by_key(|m| m.tag.to_string());
        models_disp.sort_by_key(|m| m.tag.to_string());
        events_disp.sort_by_key(|m| m.tag.to_string());
        libraries_disp.sort_by_key(|m| m.tag.to_string());
        external_contracts_disp.sort_by_key(|c| format!("{}-{}", c.contract_name, c.tag));

        print_table(&namespaces_disp, Some(Color::FG_BRIGHT_BLACK), None);
        print_table(&contracts_disp, Some(Color::FG_BRIGHT_BLACK), None);
        print_table(&libraries_disp, Some(Color::FG_BRIGHT_BLACK), None);
        print_table(&models_disp, Some(Color::FG_BRIGHT_BLACK), None);
        print_table(&events_disp, Some(Color::FG_BRIGHT_BLACK), None);
        print_table(&external_contracts_disp, Some(Color::FG_BRIGHT_BLACK), None);
    }
}

/// Displays the resource diff with the address and class hash.
fn resource_diff_display(world_diff: &WorldDiff, resource: &ResourceDiff) -> ResourceInspect {
    let n_local_writers_only = world_diff.get_writers(resource.dojo_selector()).only_local().len();
    let n_local_owners_only = world_diff.get_owners(resource.dojo_selector()).only_local().len();
    // Dirty perms is pertinent only when the status is synced.
    let has_dirty_perms = n_local_writers_only > 0 || n_local_owners_only > 0;

    match resource.resource_type() {
        ResourceType::Namespace => {
            let status = match resource {
                ResourceDiff::Created(_) => ResourceStatus::Created,
                ResourceDiff::Synced(_, _) => {
                    if has_dirty_perms {
                        ResourceStatus::DirtyLocalPerms
                    } else {
                        ResourceStatus::Synced
                    }
                }
                _ => unreachable!(),
            };

            let status = if world_diff.profile_config.is_skipped(&resource.tag()) {
                ResourceStatus::MigrationSkipped
            } else {
                status
            };

            ResourceInspect::Namespace(NamespaceInspect {
                name: resource.name(),
                status,
                selector: format!("{:#066x}", resource.dojo_selector()),
            })
        }
        ResourceType::Contract => {
            let (is_initialized, contract_address, status) = match resource {
                ResourceDiff::Created(_) => (
                    false,
                    world_diff.get_contract_address(resource.dojo_selector()).unwrap(),
                    ResourceStatus::Created,
                ),
                ResourceDiff::Updated(_, remote) => (
                    remote.as_contract_or_panic().is_initialized,
                    remote.address(),
                    ResourceStatus::Updated,
                ),
                ResourceDiff::Synced(_, remote) => (
                    remote.as_contract_or_panic().is_initialized,
                    remote.address(),
                    if has_dirty_perms {
                        ResourceStatus::DirtyLocalPerms
                    } else {
                        ResourceStatus::Synced
                    },
                ),
            };

            let status = if world_diff.profile_config.is_skipped(&resource.tag()) {
                ResourceStatus::MigrationSkipped
            } else {
                status
            };

            ResourceInspect::Contract(ContractInspect {
                tag: resource.tag(),
                status,
                is_initialized,
                address: format!("{:#066x}", contract_address),
                current_class_hash: format!("{:#066x}", resource.current_class_hash()),
                selector: format!("{:#066x}", resource.dojo_selector()),
            })
        }
        ResourceType::Library => {
            let (_current_class_hash, status) = match resource {
                ResourceDiff::Created(_) => {
                    (resource.current_class_hash(), ResourceStatus::Created)
                }
                ResourceDiff::Updated(_, _remote) => {
                    (resource.current_class_hash(), ResourceStatus::Updated)
                }
                ResourceDiff::Synced(_, remote) => (
                    remote.current_class_hash(),
                    if has_dirty_perms {
                        ResourceStatus::DirtyLocalPerms
                    } else {
                        ResourceStatus::Synced
                    },
                ),
            };

            let status = if world_diff.profile_config.is_skipped(&resource.tag()) {
                ResourceStatus::MigrationSkipped
            } else {
                status
            };

            let version = world_diff
                .profile_config
                .lib_versions
                .as_ref()
                .expect("expected lib_versions")
                .get(&resource.tag())
                .expect("lib_version not found");

            ResourceInspect::Library(LibraryInspect {
                tag: resource.tag(),
                status,
                current_class_hash: format!("{:#066x}", resource.current_class_hash()),
                selector: format!("{:#066x}", resource.dojo_selector()),
                version: version.to_string(),
            })
        }
        ResourceType::Model => {
            let status = match resource {
                ResourceDiff::Created(_) => ResourceStatus::Created,
                ResourceDiff::Updated(_, _) => ResourceStatus::Updated,
                ResourceDiff::Synced(_, _) => {
                    if has_dirty_perms {
                        ResourceStatus::DirtyLocalPerms
                    } else {
                        ResourceStatus::Synced
                    }
                }
            };

            let status = if world_diff.profile_config.is_skipped(&resource.tag()) {
                ResourceStatus::MigrationSkipped
            } else {
                status
            };

            ResourceInspect::Model(ModelInspect {
                tag: resource.tag(),
                status,
                selector: format!("{:#066x}", resource.dojo_selector()),
            })
        }
        ResourceType::Event => {
            let status = match resource {
                ResourceDiff::Created(_) => ResourceStatus::Created,
                ResourceDiff::Updated(_, _) => ResourceStatus::Updated,
                ResourceDiff::Synced(_, _) => {
                    if has_dirty_perms {
                        ResourceStatus::DirtyLocalPerms
                    } else {
                        ResourceStatus::Synced
                    }
                }
            };

            let status = if world_diff.profile_config.is_skipped(&resource.tag()) {
                ResourceStatus::MigrationSkipped
            } else {
                status
            };

            ResourceInspect::Event(EventInspect {
                tag: resource.tag(),
                status,
                selector: format!("{:#066x}", resource.dojo_selector()),
            })
        }
        ResourceType::ExternalContract => {
            let (external_contract, contract_address, status) = match resource {
                ResourceDiff::Created(local) => {
                    let local = local.as_external_contract().unwrap();
                    let address = match local {
                        ExternalContractLocal::SozoManaged(l) => l.computed_address,
                        ExternalContractLocal::SelfManaged(l) => l.contract_address,
                    };
                    (local, address, ResourceStatus::Created)
                }
                ResourceDiff::Updated(local, remote) => {
                    let local = local.as_external_contract().unwrap();
                    let remote = remote.as_external_contract_or_panic();
                    (local, remote.common.address, ResourceStatus::Updated)
                }
                ResourceDiff::Synced(local, remote) => {
                    let local = local.as_external_contract().unwrap();
                    let remote = remote.as_external_contract_or_panic();
                    (local, remote.common.address, ResourceStatus::Synced)
                }
            };

            let status = if world_diff.profile_config.is_skipped(&resource.tag()) {
                ResourceStatus::MigrationSkipped
            } else {
                status
            };

            let contract_name = match external_contract {
                ExternalContractLocal::SozoManaged(c) => c.contract_name.clone(),
                ExternalContractLocal::SelfManaged(c) => c.name.clone(),
            };

            ResourceInspect::ExternalContract(ExternalContractInspect {
                contract_name,
                tag: resource.tag(),
                status,
                address: format!("{:#066x}", contract_address),
                class_hash: format!("{:#066x}", resource.current_class_hash()),
                selector: format!("{:#066x}", resource.dojo_selector()),
            })
        }
    }
}

/// Prints a table.
fn print_table<T>(data: T, color: Option<Color>, title: Option<&str>)
where
    T: IntoIterator + Clone,
    <T as IntoIterator>::Item: Tabled,
{
    if data.clone().into_iter().count() == 0 {
        return;
    }

    let mut table = Table::new(data);
    table.with(Style::psql());

    if let Some(color) = color {
        table.modify(Cell::new(0, 0), color);
    }

    if let Some(title) = title {
        println!("{title}");
    }

    println!("{table}\n");
}

/// Pretty prints a section header
fn print_section_header(str: &str) {
    println!("\n{}", str.blue());
}

/// Pretty prints a TOML string.
fn pretty_print_toml(str: &str) {
    for line in str.lines() {
        if line.starts_with("[") {
            print_section_header(line);
        } else if line.contains('=') {
            // Print key-value pairs with keys in green and values.
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() == 2 {
                let key = parts[0].trim();
                let value = parts[1].trim().replace("\"", "");

                let colored_values = match key {
                    "status" => match value.to_string().as_str() {
                        "Created" => value.blue(),
                        "Updated" => value.yellow(),
                        "Synced" => value.green(),
                        "DirtyLocalPerms" => "Dirty local permissions".yellow(),
                        _ => value.white(),
                    },
                    "is_initialized" => match value.to_string().as_str() {
                        "true" => value.green(),
                        "false" => value.red(),
                        _ => value.white(),
                    },
                    _ => value.white(),
                };

                println!("{}: {}", key.bright_black(), colored_values);
            } else {
                println!("{}", line);
            }
        } else {
            // Print other lines normally.
            println!("{}", line);
        }
    }
}
