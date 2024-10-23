//! Converts Scarb artifacts to local resources.

use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json;
use starknet::core::types::contract::{AbiEntry, AbiImpl, SierraClass};

use super::{ContractLocal, EventLocal, ModelLocal, WorldLocal};

const WORLD_INTF: &str = "dojo::world::iworld::IWorld";
const CONTRACT_INTF: &str = "dojo::contract::interface::IContract";
const MODEL_INTF: &str = "dojo::model::interface::IModel";
const EVENT_INTF: &str = "dojo::event::interface::IEvent";

impl WorldLocal {
    pub fn from_directory<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let mut world = Self::default();
        world.parse_directory(dir)?;
        Ok(world)
    }

    /// Parses a directory and processes each file to identify it a Dojo resource.
    fn parse_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Ok(sierra) =
                    serde_json::from_reader::<_, SierraClass>(std::fs::File::open(&path)?)
                {
                    let abi = sierra.abi.clone();

                    let impls = abi
                        .iter()
                        .filter_map(|e| if let AbiEntry::Impl(i) = e { Some(i) } else { None })
                        .collect::<Vec<_>>();

                    for i in impls {
                        match identify_resource_type(i) {
                            ResourceType::World => {
                                self.class = Some(sierra);
                                break;
                            }
                            ResourceType::Contract(name) => {
                                self.contracts.insert(name, ContractLocal { class: sierra });
                                break;
                            }
                            ResourceType::Model(name) => {
                                self.models.insert(name, ModelLocal { class: sierra });
                                break;
                            }
                            ResourceType::Event(name) => {
                                self.events.insert(name, EventLocal { class: sierra });
                                break;
                            }
                            ResourceType::Other => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// A simple enum to identify the type of resource with their name.
#[derive(Debug, PartialEq)]
enum ResourceType {
    World,
    Contract(String),
    Model(String),
    Event(String),
    Other,
}

/// Identifies the type of resource from the ABI implementation.
fn identify_resource_type(implem: &AbiImpl) -> ResourceType {
    if implem.interface_name == WORLD_INTF {
        ResourceType::World
    } else if implem.interface_name == CONTRACT_INTF {
        ResourceType::Contract(name_from_impl(&implem.name))
    } else if implem.interface_name == MODEL_INTF {
        ResourceType::Model(name_from_impl(&implem.name))
    } else if implem.interface_name == EVENT_INTF {
        ResourceType::Event(name_from_impl(&implem.name))
    } else {
        ResourceType::Other
    }
}

/// Extract the contract name from the `IContract`/`IModel`/`IEvent` implementation.
///
/// Dojo lang always output the implementation with the name of the contract itself, with
/// a double underscore as separator.
fn name_from_impl(impl_name: &str) -> String {
    impl_name.split("__").collect::<Vec<&str>>()[0].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_from_impl() {
        assert_eq!(name_from_impl("contract__MyImpl"), "contract");
        assert_eq!(name_from_impl("Model__MyModel"), "Model");
        assert_eq!(name_from_impl("Event__MyEvent"), "Event");
    }

    #[test]
    fn test_identify_resource_type() {
        assert_eq!(
            identify_resource_type(&AbiImpl {
                interface_name: WORLD_INTF.to_string(),
                name: "IWorld".to_string()
            }),
            ResourceType::World
        );

        assert_eq!(
            identify_resource_type(&AbiImpl {
                interface_name: CONTRACT_INTF.to_string(),
                name: "contract__DojoModelImpl".to_string()
            }),
            ResourceType::Contract("contract".to_string())
        );

        assert_eq!(
            identify_resource_type(&AbiImpl {
                interface_name: MODEL_INTF.to_string(),
                name: "Model__DojoModelImpl".to_string()
            }),
            ResourceType::Model("Model".to_string())
        );

        assert_eq!(
            identify_resource_type(&AbiImpl {
                interface_name: EVENT_INTF.to_string(),
                name: "Event__DojoEventImpl".to_string()
            }),
            ResourceType::Event("Event".to_string())
        );
    }

    #[ignore = "The simple example must be stabilized first (and built for this test to work)"]
    #[test]
    fn test_load_world_from_directory() {
        let world =
            WorldLocal::from_directory("/Users/glihm/cgg/dojo/examples/simple/target/dev").unwrap();
        assert_eq!(world.class.is_some(), true);
        assert_eq!(world.contracts.len(), 1);
        assert_eq!(world.models.len(), 1);
        assert_eq!(world.contracts.get("c1").is_some(), true);
        assert_eq!(world.models.get("M").is_some(), true);
    }
}
