//! Fetches the events for the given world address and converts them to remote resources.
//!
//! The world is responsible for managing the remote resources onchain. We are expected
//! to safely unwrap the resources lookup as they are supposed to exist.
//!
//! Events are also sequential, a resource is not expected to be upgraded before
//! being registered. We take advantage of this fact to optimize the data gathering.

use std::collections::HashSet;

use anyhow::Result;
use starknet::core::types::{BlockId, BlockTag, EventFilter, Felt, StarknetError};
use starknet::macros::felt;
use starknet::providers::{Provider, ProviderError};
use tracing::{debug, trace};

use super::permissions::PermissionsUpdateable;
use super::{ResourceRemote, WorldRemote};
use crate::constants::WORLD;
use crate::contracts::abigen::world::{self, Event as WorldEvent};
use crate::remote::{
    CommonRemoteInfo, ContractRemote, EventRemote, ExternalContractRemote, LibraryRemote,
    ModelRemote, NamespaceRemote,
};

impl WorldRemote {
    /// Fetch the events from the world and convert them to remote resources.
    ///
    /// The `max_block_range` is the maximum number of blocks that will separate the `from_block`
    /// and the `to_block` in the event fetching, which if too high will cause the event fetching
    /// to fail in most of the node providers.
    #[allow(clippy::field_reassign_with_default)]
    pub async fn from_events<P: Provider>(
        world_address: Felt,
        provider: &P,
        from_block: Option<u64>,
        max_block_range: u64,
        whitelisted_namespaces: Option<Vec<String>>,
    ) -> Result<Self> {
        let mut world = Self::default();

        world.address = world_address;

        let chain_id = provider.chain_id().await?;
        // Katana if it's not `SN_SEPOLIA` or `SN_MAIN`.
        let is_katana =
            chain_id != felt!("0x534e5f5345504f4c4941") && chain_id != felt!("0x534e5f4d41494e");

        match provider.get_class_hash_at(BlockId::Tag(BlockTag::PreConfirmed), world_address).await
        {
            Ok(_) => {
                // The world contract exists, we can continue and fetch the events.
            }
            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                trace!(%world_address, "No remote world contract found.");
                return Ok(world);
            }
            Err(e) => return Err(e.into()),
        };

        // We only care about management events, not resource events (set, delete, emit).
        let keys = vec![vec![
            world::WorldSpawned::event_selector(),
            world::WorldUpgraded::event_selector(),
            world::NamespaceRegistered::event_selector(),
            world::ModelRegistered::event_selector(),
            world::EventRegistered::event_selector(),
            world::ContractRegistered::event_selector(),
            world::ExternalContractRegistered::event_selector(),
            world::ModelUpgraded::event_selector(),
            world::EventUpgraded::event_selector(),
            world::ContractUpgraded::event_selector(),
            world::ExternalContractUpgraded::event_selector(),
            world::ContractInitialized::event_selector(),
            world::WriterUpdated::event_selector(),
            world::OwnerUpdated::event_selector(),
            world::MetadataUpdate::event_selector(),
            world::LibraryRegistered::event_selector(),
        ]];

        let chunk_size = 500;

        let from_block = from_block.unwrap_or(0);
        let to_block = provider.block_number().await?;
        let mut current_from = from_block;
        let mut events = Vec::new();

        while current_from <= to_block {
            let current_to = std::cmp::min(current_from + max_block_range - 1, to_block);

            let filter = EventFilter {
                from_block: Some(BlockId::Number(current_from)),
                to_block: Some(BlockId::Number(current_to)),
                address: Some(world_address),
                keys: Some(keys.clone()),
            };

            trace!(
                world_address = format!("{:#066x}", world_address),
                chunk_size,
                ?filter,
                "Fetching remote world events for block range {}-{}.",
                current_from,
                current_to
            );

            let mut continuation_token = None;
            loop {
                let page =
                    provider.get_events(filter.clone(), continuation_token, chunk_size).await?;

                if is_katana && page.events.is_empty() {
                    break;
                }

                events.extend(page.events);

                continuation_token = page.continuation_token;
                if continuation_token.is_none() {
                    break;
                }
            }

            current_from = current_to + 1;
        }

        trace!(
            events_count = events.len(),
            world_address = format!("{:#066x}", world_address),
            "Fetched events for world."
        );

        for event in &events {
            match world::Event::try_from(event) {
                Ok(ev) => {
                    trace!(?ev, "Processing world event.");
                    world.match_event(ev, &whitelisted_namespaces)?;
                }
                Err(e) => {
                    tracing::error!(
                        ?e,
                        "Failed to parse remote world event which is supposed to be valid."
                    );
                }
            }
        }

        Ok(world)
    }

    /// Matches the given event to the corresponding remote resource and inserts it into the world.
    fn match_event(
        &mut self,
        event: WorldEvent,
        whitelisted_namespaces: &Option<Vec<String>>,
    ) -> Result<()> {
        match event {
            WorldEvent::WorldSpawned(e) => {
                self.class_hashes.push(e.class_hash.into());

                // The creator is the world's owner, but no event emitted for that.
                self.external_owners.insert(WORLD, HashSet::from([e.creator.into()]));

                trace!(
                    class_hash = format!("{:#066x}", e.class_hash.0),
                    creator = format!("{:#066x}", e.creator.0),
                    "World spawned."
                );
            }
            WorldEvent::WorldUpgraded(e) => {
                self.class_hashes.push(e.class_hash.into());

                trace!(class_hash = format!("{:#066x}", e.class_hash.0), "World upgraded.");
            }
            WorldEvent::NamespaceRegistered(e) => {
                let r = ResourceRemote::Namespace(NamespaceRemote::new(e.namespace.to_string()?));

                if is_whitelisted(whitelisted_namespaces, &e.namespace.to_string()?) {
                    trace!(?r, "Namespace registered.");
                    self.add_resource(r);
                } else {
                    debug!(namespace = e.namespace.to_string()?, "Namespace not whitelisted.");
                }
            }
            WorldEvent::ModelRegistered(e) => {
                let namespace = e.namespace.to_string()?;

                if !is_whitelisted(whitelisted_namespaces, &namespace) {
                    debug!(
                        namespace,
                        model = e.name.to_string()?,
                        "Model's namespace not whitelisted."
                    );

                    return Ok(());
                }

                let r = ResourceRemote::Model(ModelRemote {
                    common: CommonRemoteInfo::new(
                        e.class_hash.into(),
                        &e.namespace.to_string()?,
                        &e.name.to_string()?,
                        e.address.into(),
                    ),
                });
                trace!(?r, "Model registered.");

                self.add_resource(r);
            }
            WorldEvent::EventRegistered(e) => {
                let namespace = e.namespace.to_string()?;

                if !is_whitelisted(whitelisted_namespaces, &namespace) {
                    debug!(
                        namespace,
                        event = e.name.to_string()?,
                        "Event's namespace not whitelisted."
                    );
                    return Ok(());
                }

                let r = ResourceRemote::Event(EventRemote {
                    common: CommonRemoteInfo::new(
                        e.class_hash.into(),
                        &e.namespace.to_string()?,
                        &e.name.to_string()?,
                        e.address.into(),
                    ),
                });
                trace!(?r, "Event registered.");

                self.add_resource(r);
            }
            WorldEvent::ContractRegistered(e) => {
                let namespace = e.namespace.to_string()?;

                if !is_whitelisted(whitelisted_namespaces, &namespace) {
                    debug!(
                        namespace,
                        contract = e.name.to_string()?,
                        "Contract's namespace not whitelisted."
                    );

                    return Ok(());
                }

                let r = ResourceRemote::Contract(ContractRemote {
                    common: CommonRemoteInfo::new(
                        e.class_hash.into(),
                        &namespace,
                        &e.name.to_string()?,
                        e.address.into(),
                    ),
                    is_initialized: false,
                });
                trace!(?r, "Contract registered.");

                self.add_resource(r);
            }
            WorldEvent::ExternalContractRegistered(e) => {
                let namespace = e.namespace.to_string()?;

                if !is_whitelisted(whitelisted_namespaces, &namespace) {
                    debug!(
                        namespace,
                        contract_name = e.contract_name.to_string()?,
                        instance_name = e.instance_name.to_string()?,
                        "External contract's namespace not whitelisted."
                    );

                    return Ok(());
                }

                let r = ResourceRemote::ExternalContract(ExternalContractRemote {
                    common: CommonRemoteInfo::new(
                        e.class_hash.into(),
                        &namespace,
                        &e.instance_name.to_string()?,
                        e.contract_address.into(),
                    ),
                    block_number: e.block_number,
                });
                trace!(?r, "External contract registered.");

                self.add_resource(r);
            }
            WorldEvent::LibraryRegistered(e) => {
                let namespace = e.namespace.to_string()?;

                if !is_whitelisted(whitelisted_namespaces, &namespace) {
                    debug!(
                        namespace,
                        contract = e.name.to_string()?,
                        "Library's namespace not whitelisted."
                    );

                    return Ok(());
                }

                let full_name = e.name.to_string().unwrap();
                let version = full_name.split(&"_v").last().expect("expected version");
                let name = full_name.replace(&format!("_v{}", version), "");
                let r = ResourceRemote::Library(LibraryRemote {
                    common: CommonRemoteInfo::new(
                        e.class_hash.into(),
                        &namespace,
                        &name.to_string(),
                        Felt::ZERO,
                    ),
                    version: version.to_string(),
                });
                trace!(?r, "Library registered.");

                self.add_resource(r);
            }
            WorldEvent::ModelUpgraded(e) => {
                let resource = if let Some(resource) = self.resources.get_mut(&e.selector) {
                    resource
                } else {
                    debug!(
                        selector = format!("{:#066x}", e.selector),
                        "Model not found (may be excluded by whitelist of namespaces)."
                    );

                    return Ok(());
                };
                trace!(?resource, "Model upgraded.");

                resource.push_class_hash(e.class_hash.into());
            }
            WorldEvent::EventUpgraded(e) => {
                let resource = if let Some(resource) = self.resources.get_mut(&e.selector) {
                    resource
                } else {
                    debug!(
                        selector = format!("{:#066x}", e.selector),
                        "Event not found (may be excluded by whitelist of namespaces)."
                    );

                    return Ok(());
                };
                trace!(?resource, "Event upgraded.");

                resource.push_class_hash(e.class_hash.into());
            }
            WorldEvent::ContractUpgraded(e) => {
                let resource = if let Some(resource) = self.resources.get_mut(&e.selector) {
                    resource
                } else {
                    debug!(
                        selector = format!("{:#066x}", e.selector),
                        "Contract not found (may be excluded by whitelist of namespaces)."
                    );

                    return Ok(());
                };
                trace!(?resource, "Contract upgraded.");

                resource.push_class_hash(e.class_hash.into());
            }
            WorldEvent::ExternalContractUpgraded(e) => {
                let resource = if let Some(resource) = self.resources.get_mut(&e.contract_selector)
                {
                    resource
                } else {
                    debug!(
                        selector = format!("{:#066x}", e.contract_selector),
                        "External contract not found (may be excluded by whitelist of namespaces)."
                    );

                    return Ok(());
                };
                trace!(?resource, "External contract upgraded.");

                if let ResourceRemote::ExternalContract(r) = resource {
                    r.block_number = e.block_number;
                }

                resource.push_class_hash(e.class_hash.into());
            }
            WorldEvent::ContractInitialized(e) => {
                let resource = if let Some(resource) = self.resources.get_mut(&e.selector) {
                    resource
                } else {
                    debug!(
                        selector = format!("{:#066x}", e.selector),
                        "Contract not found (may be excluded by whitelist of namespaces)."
                    );

                    return Ok(());
                };

                let contract = resource.as_contract_mut()?;
                contract.is_initialized = true;

                trace!(
                    selector = format!("{:#066x}", e.selector),
                    init_calldata = format!("{:?}", e.init_calldata),
                    "Contract initialized."
                );
            }
            WorldEvent::WriterUpdated(e) => {
                // The resource may not be managed by the local project.
                if let Some(resource) = self.resources.get_mut(&e.resource) {
                    resource.update_writer(e.contract.into(), e.value)?;
                } else {
                    let entry = self.external_writers.entry(e.resource).or_default();

                    if e.value {
                        entry.insert(e.contract.into());
                    } else {
                        entry.remove(&e.contract.into());
                    }
                }

                trace!(?e, "Writer updated.");
            }
            WorldEvent::OwnerUpdated(e) => {
                // The resource may not be managed by the local project.
                if let Some(resource) = self.resources.get_mut(&e.resource) {
                    resource.update_owner(e.contract.into(), e.value)?;
                } else {
                    let entry = self.external_owners.entry(e.resource).or_default();

                    if e.value {
                        entry.insert(e.contract.into());
                    } else {
                        entry.remove(&e.contract.into());
                    }
                }

                trace!(?e, "Owner updated.");
            }
            WorldEvent::MetadataUpdate(e) => {
                if e.resource == WORLD {
                    self.metadata_hash = e.hash;
                } else {
                    // Unwrap is safe because the resource must exist in the world.
                    let resource = self.resources.get_mut(&e.resource).unwrap();
                    trace!(?resource, "Metadata updated.");

                    resource.set_metadata_hash(e.hash);
                }
            }
            _ => {
                // Ignore events filtered out by the event filter.
            }
        }

        Ok(())
    }
}

/// Returns true if the namespace is whitelisted, false otherwise.
/// If no whitelist is provided, all namespaces are considered whitelisted.
#[inline]
fn is_whitelisted(whitelisted_namespaces: &Option<Vec<String>>, namespace: &str) -> bool {
    if let Some(namespaces) = whitelisted_namespaces {
        return namespaces.contains(&namespace.to_string());
    }

    true
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use cainome::cairo_serde::ByteArray;
    use dojo_types::naming;

    use super::*;

    const NO_WHITELIST: Option<Vec<String>> = None;

    #[tokio::test]
    async fn test_world_spawned_event() {
        let mut world_remote = WorldRemote::default();
        let event = WorldEvent::WorldSpawned(world::WorldSpawned {
            class_hash: Felt::ONE.into(),
            creator: Felt::ONE.into(),
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();
        assert_eq!(world_remote.class_hashes.len(), 1);
    }

    #[tokio::test]
    async fn test_world_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let event =
            WorldEvent::WorldUpgraded(world::WorldUpgraded { class_hash: Felt::ONE.into() });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();
        assert_eq!(world_remote.class_hashes.len(), 1);
    }

    #[tokio::test]
    async fn test_namespace_registered_event() {
        let mut world_remote = WorldRemote::default();
        let event = WorldEvent::NamespaceRegistered(world::NamespaceRegistered {
            namespace: ByteArray::from_string("ns").unwrap(),
            hash: 123.into(),
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let selector = naming::compute_bytearray_hash("ns");
        assert!(world_remote.resources.contains_key(&selector));

        let resource = world_remote.resources.get(&selector).unwrap();
        assert!(matches!(resource, ResourceRemote::Namespace(_)));
    }

    #[tokio::test]
    async fn test_namespace_registered_event_not_whitelisted() {
        let mut world_remote = WorldRemote::default();
        let event = WorldEvent::NamespaceRegistered(world::NamespaceRegistered {
            namespace: ByteArray::from_string("ns").unwrap(),
            hash: 123.into(),
        });

        world_remote.match_event(event, &Some(vec!["ns2".to_string()])).unwrap();

        let selector = naming::compute_bytearray_hash("ns");
        assert!(!world_remote.resources.contains_key(&selector));
    }

    #[tokio::test]
    async fn test_model_registered_event() {
        let mut world_remote = WorldRemote::default();
        let event = WorldEvent::ModelRegistered(world::ModelRegistered {
            class_hash: Felt::ONE.into(),
            name: ByteArray::from_string("m").unwrap(),
            address: Felt::ONE.into(),
            namespace: ByteArray::from_string("ns").unwrap(),
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();
        let selector = naming::compute_selector_from_names("ns", "m");
        assert!(world_remote.resources.contains_key(&selector));

        let resource = world_remote.resources.get(&selector).unwrap();
        assert!(matches!(resource, ResourceRemote::Model(_)));
    }

    #[tokio::test]
    async fn test_event_registered_event() {
        let mut world_remote = WorldRemote::default();
        let event = WorldEvent::EventRegistered(world::EventRegistered {
            class_hash: Felt::ONE.into(),
            name: ByteArray::from_string("e").unwrap(),
            address: Felt::ONE.into(),
            namespace: ByteArray::from_string("ns").unwrap(),
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();
        let selector = naming::compute_selector_from_names("ns", "e");
        assert!(world_remote.resources.contains_key(&selector));

        let resource = world_remote.resources.get(&selector).unwrap();
        assert!(matches!(resource, ResourceRemote::Event(_)));
    }

    #[tokio::test]
    async fn test_contract_registered_event() {
        let mut world_remote = WorldRemote::default();
        let event = WorldEvent::ContractRegistered(world::ContractRegistered {
            class_hash: Felt::ONE.into(),
            name: ByteArray::from_string("c").unwrap(),
            address: Felt::ONE.into(),
            namespace: ByteArray::from_string("ns").unwrap(),
            salt: Felt::ONE,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();
        let selector = naming::compute_selector_from_names("ns", "c");
        assert!(world_remote.resources.contains_key(&selector));

        let resource = world_remote.resources.get(&selector).unwrap();
        assert!(matches!(resource, ResourceRemote::Contract(_)));
    }

    #[tokio::test]
    async fn test_model_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "m");

        let resource = ResourceRemote::Model(ModelRemote {
            common: CommonRemoteInfo::new(Felt::ONE, "ns", "m", Felt::ONE),
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::ModelUpgraded(world::ModelUpgraded {
            selector,
            class_hash: Felt::TWO.into(),
            address: Felt::ONE.into(),
            prev_address: Felt::ONE.into(),
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_model_or_panic().common.class_hashes, vec![Felt::ONE, Felt::TWO]);
    }

    #[tokio::test]
    async fn test_event_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "e");

        let resource = ResourceRemote::Event(EventRemote {
            common: CommonRemoteInfo::new(Felt::ONE, "ns", "e", Felt::ONE),
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::EventUpgraded(world::EventUpgraded {
            selector,
            class_hash: Felt::TWO.into(),
            address: Felt::ONE.into(),
            prev_address: Felt::ONE.into(),
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_event_or_panic().common.class_hashes, vec![Felt::ONE, Felt::TWO]);
    }

    #[tokio::test]
    async fn test_contract_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "c");

        let resource = ResourceRemote::Contract(ContractRemote {
            common: CommonRemoteInfo::new(Felt::ONE, "ns", "c", Felt::ONE),
            is_initialized: false,
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::ContractUpgraded(world::ContractUpgraded {
            selector,
            class_hash: Felt::TWO.into(),
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();
        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_contract_or_panic().common.class_hashes, vec![Felt::ONE, Felt::TWO]);
    }

    #[tokio::test]
    async fn test_contract_initialized_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "c");

        let resource = ResourceRemote::Contract(ContractRemote {
            common: CommonRemoteInfo::new(Felt::ONE, "ns", "c", Felt::ONE),
            is_initialized: false,
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::ContractInitialized(world::ContractInitialized {
            selector,
            init_calldata: vec![],
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert!(resource.as_contract_or_panic().is_initialized);
    }

    #[tokio::test]
    async fn test_writer_updated_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_bytearray_hash("ns");

        let resource = ResourceRemote::Namespace(NamespaceRemote::new("ns".to_string()));
        world_remote.add_resource(resource);

        let event = WorldEvent::WriterUpdated(world::WriterUpdated {
            resource: selector,
            contract: Felt::ONE.into(),
            value: true,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_namespace_or_panic().writers, HashSet::from([Felt::ONE]));

        let event = WorldEvent::WriterUpdated(world::WriterUpdated {
            resource: selector,
            contract: Felt::ONE.into(),
            value: false,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_namespace_or_panic().writers, HashSet::from([]));
    }

    #[tokio::test]
    async fn test_owner_updated_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_bytearray_hash("ns");

        let resource = ResourceRemote::Namespace(NamespaceRemote::new("ns".to_string()));
        world_remote.add_resource(resource);

        let event = WorldEvent::OwnerUpdated(world::OwnerUpdated {
            resource: selector,
            contract: Felt::ONE.into(),
            value: true,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_namespace_or_panic().owners, HashSet::from([Felt::ONE]));

        let event = WorldEvent::OwnerUpdated(world::OwnerUpdated {
            resource: selector,
            contract: Felt::ONE.into(),
            value: false,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_namespace_or_panic().owners, HashSet::from([]));
    }

    #[tokio::test]
    async fn test_metadata_updated_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "m1");

        let resource = ResourceRemote::Model(ModelRemote {
            common: CommonRemoteInfo::new(Felt::TWO, "ns", "m1", Felt::ONE),
        });
        world_remote.add_resource(resource);

        let event = WorldEvent::MetadataUpdate(world::MetadataUpdate {
            resource: selector,
            uri: ByteArray::from_string("ipfs://m1").unwrap(),
            hash: Felt::THREE,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.metadata_hash(), Felt::THREE);

        let event = WorldEvent::MetadataUpdate(world::MetadataUpdate {
            resource: selector,
            uri: ByteArray::from_string("ipfs://m1").unwrap(),
            hash: Felt::ONE,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.metadata_hash(), Felt::ONE);
    }

    #[tokio::test]
    async fn test_external_contract_registered_event() {
        let mut world_remote = WorldRemote::default();

        let contract_selector = naming::compute_selector_from_names("ns", "c1");
        let event = WorldEvent::ExternalContractRegistered(world::ExternalContractRegistered {
            namespace: ByteArray::from_string("ns").unwrap(),
            contract_name: ByteArray::from_string("NOT_USED").unwrap(),
            instance_name: ByteArray::from_string("c1").unwrap(),
            contract_selector,
            class_hash: Felt::TWO.into(),
            contract_address: Felt::ONE.into(),
            block_number: 123,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        assert!(world_remote.resources.contains_key(&contract_selector));

        let resource = world_remote.resources.get(&contract_selector).unwrap();
        let contract = resource.as_external_contract_or_panic();

        assert_eq!(contract.common.name, "c1".to_string(), "bad name");
        assert_eq!(contract.common.namespace, "ns".to_string(), "bad namespace");
        assert_eq!(contract.common.address, Felt::ONE, "bad address");
        assert_eq!(contract.common.class_hashes, vec![Felt::TWO], "bad classhash");
    }

    #[tokio::test]
    async fn test_external_contract_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let contract_selector = naming::compute_selector_from_names("ns", "c1");

        let resource = ResourceRemote::ExternalContract(ExternalContractRemote {
            common: CommonRemoteInfo::new(Felt::ONE, "ns", "c1", Felt::ONE),
            block_number: 123,
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::ExternalContractUpgraded(world::ExternalContractUpgraded {
            namespace: ByteArray::from_string("ns").unwrap(),
            instance_name: ByteArray::from_string("c1").unwrap(),
            contract_selector,
            class_hash: Felt::TWO.into(),
            contract_address: Felt::ONE.into(),
            block_number: 123,
        });

        world_remote.match_event(event, &NO_WHITELIST).unwrap();

        let resource = world_remote.resources.get(&contract_selector).unwrap();
        let external_contract = resource.as_external_contract_or_panic();
        assert_eq!(external_contract.common.class_hashes, vec![Felt::ONE, Felt::TWO]);
        assert_eq!(external_contract.block_number, 123);
    }
}
