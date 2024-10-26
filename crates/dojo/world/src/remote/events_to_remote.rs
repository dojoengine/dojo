//! Fetches the events for the given world address and converts them to remote resources.
//!
//! The world is responsible for managing the remote resources onchain. We are expected
//! to safely unwrap the resources lookup as they are supposed to exist.
//!
//! Events are also sequential, a resource is not expected to be upgraded before
//! being registered. We take advantage of this fact to optimize the data gathering.

use anyhow::Result;
use starknet::core::types::{BlockId, BlockTag, EventFilter, Felt, StarknetError};
use starknet::providers::{Provider, ProviderError};
use tracing::trace;

use super::permissions::PermissionsUpdateable;
use super::{ResourceRemote, WorldRemote};
use crate::contracts::abigen::world::{self, Event as WorldEvent};
use crate::remote::{
    CommonResourceRemoteInfo, ContractRemote, EventRemote, ModelRemote, NamespaceRemote,
};

impl WorldRemote {
    /// Fetch the events from the world and convert them to remote resources.
    pub async fn from_events<P: Provider>(world_address: Felt, provider: &P) -> Result<Self> {
        let mut world = Self::default();

        match provider.get_class_hash_at(BlockId::Tag(BlockTag::Pending), world_address).await {
            Ok(_) => {
                // The world contract exists, we can continue and fetch the events.
            }
            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                tracing::trace!(%world_address, "No remote world contract found.");
                return Ok(world);
            }
            Err(e) => return Err(e.into()),
        };

        // We only care about management events, not resource events (set, delete, emit).
        let keys = vec![vec![
            world::WorldSpawned::selector(),
            world::WorldUpgraded::selector(),
            world::NamespaceRegistered::selector(),
            world::ModelRegistered::selector(),
            world::EventRegistered::selector(),
            world::ContractRegistered::selector(),
            world::ModelUpgraded::selector(),
            world::EventUpgraded::selector(),
            world::ContractUpgraded::selector(),
            world::ContractInitialized::selector(),
            world::WriterUpdated::selector(),
            world::OwnerUpdated::selector(),
        ]];

        let filter = EventFilter {
            from_block: None,
            to_block: Some(BlockId::Tag(BlockTag::Pending)),
            address: Some(world_address),
            keys: Some(keys),
        };

        let chunk_size = 500;

        tracing::trace!(
            world_address = format!("{:#066x}", world_address),
            chunk_size,
            ?filter,
            "Fetching remote world events."
        );

        let mut events = Vec::new();

        // Initial fetch.
        let page = provider.get_events(filter.clone(), None, chunk_size).await?;
        events.extend(page.events);

        let mut continuation_token = page.continuation_token;

        while continuation_token.is_some() {
            let page = provider.get_events(filter.clone(), continuation_token, chunk_size).await?;

            // TODO: remove this once rebased with latest katana.
            if page.events.is_empty() {
                break;
            }

            continuation_token = page.continuation_token;
            events.extend(page.events);
        }

        for event in events {
            match world::Event::try_from(event) {
                Ok(ev) => {
                    tracing::trace!(?ev, "Processing world event.");
                    world.match_event(ev)?;
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
    fn match_event(&mut self, event: WorldEvent) -> Result<()> {
        match event {
            WorldEvent::WorldSpawned(e) => {
                self.class_hashes.push(e.class_hash.into());

                trace!(class_hash = format!("{:#066x}", e.class_hash.0), "World spawned.");
            }
            WorldEvent::WorldUpgraded(e) => {
                self.class_hashes.push(e.class_hash.into());

                trace!(class_hash = format!("{:#066x}", e.class_hash.0), "World upgraded.");
            }
            WorldEvent::NamespaceRegistered(e) => {
                let r = ResourceRemote::Namespace(NamespaceRemote::new(e.namespace.to_string()?));
                trace!(?r, "Namespace registered.");

                self.add_resource(r);
            }
            WorldEvent::ModelRegistered(e) => {
                let r = ResourceRemote::Model(ModelRemote {
                    common: CommonResourceRemoteInfo::new(
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
                let r = ResourceRemote::Event(EventRemote {
                    common: CommonResourceRemoteInfo::new(
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
                let r = ResourceRemote::Contract(ContractRemote {
                    common: CommonResourceRemoteInfo::new(
                        e.class_hash.into(),
                        &e.namespace.to_string()?,
                        &e.name.to_string()?,
                        e.address.into(),
                    ),
                    is_initialized: false,
                });
                trace!(?r, "Contract registered.");

                self.add_resource(r);
            }
            WorldEvent::ModelUpgraded(e) => {
                // Unwrap is safe because the model must exist in the world.
                let resource = self.resources.get_mut(&e.selector).unwrap();
                trace!(?resource, "Model upgraded.");

                resource.push_class_hash(e.class_hash.into());
            }
            WorldEvent::EventUpgraded(e) => {
                // Unwrap is safe because the event must exist in the world.
                let resource = self.resources.get_mut(&e.selector).unwrap();
                trace!(?resource, "Event upgraded.");

                resource.push_class_hash(e.class_hash.into());
            }
            WorldEvent::ContractUpgraded(e) => {
                // Unwrap is safe because the contract must exist in the world.
                let resource = self.resources.get_mut(&e.selector).unwrap();
                trace!(?resource, "Contract upgraded.");

                resource.push_class_hash(e.class_hash.into());
            }
            WorldEvent::ContractInitialized(e) => {
                // Unwrap is safe bcause the contract must exist in the world.
                let resource = self.resources.get_mut(&e.selector).unwrap();
                let contract = resource.as_contract_mut()?;
                contract.is_initialized = true;

                trace!(
                    selector = format!("{:#066x}", e.selector),
                    init_calldata = format!("{:?}", e.init_calldata),
                    "Contract initialized."
                );
            }
            WorldEvent::WriterUpdated(e) => {
                // Unwrap is safe because the resource must exist in the world.
                let resource = self.resources.get_mut(&e.resource).unwrap();
                resource.update_writer(e.contract.into(), e.value)?;

                trace!(?e, "Writer updated.");
            }
            WorldEvent::OwnerUpdated(e) => {
                // Unwrap is safe because the resource must exist in the world.
                let resource = self.resources.get_mut(&e.resource).unwrap();
                resource.update_owner(e.contract.into(), e.value)?;

                trace!(?e, "Owner updated.");
            }
            _ => {
                // Ignore events filtered out by the event filter.
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use cainome::cairo_serde::ByteArray;
    use dojo_types::naming;

    use super::*;

    #[tokio::test]
    async fn test_world_spawned_event() {
        let mut world_remote = WorldRemote::default();
        let event = WorldEvent::WorldSpawned(world::WorldSpawned {
            class_hash: Felt::ONE.into(),
            creator: Felt::ONE.into(),
        });

        world_remote.match_event(event).unwrap();
        assert_eq!(world_remote.class_hashes.len(), 1);
    }

    #[tokio::test]
    async fn test_world_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let event =
            WorldEvent::WorldUpgraded(world::WorldUpgraded { class_hash: Felt::ONE.into() });

        world_remote.match_event(event).unwrap();
        assert_eq!(world_remote.class_hashes.len(), 1);
    }

    #[tokio::test]
    async fn test_namespace_registered_event() {
        let mut world_remote = WorldRemote::default();
        let event = WorldEvent::NamespaceRegistered(world::NamespaceRegistered {
            namespace: ByteArray::from_string("ns").unwrap(),
            hash: 123.into(),
        });

        world_remote.match_event(event).unwrap();

        let selector = naming::compute_bytearray_hash("ns");
        assert!(world_remote.namespaces.contains(&selector));
        assert!(world_remote.resources.contains_key(&selector));

        let resource = world_remote.resources.get(&selector).unwrap();
        assert!(matches!(resource, ResourceRemote::Namespace(_)));
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

        world_remote.match_event(event).unwrap();
        let selector = naming::compute_selector_from_names("ns", "m");
        assert!(world_remote.models.get("ns").unwrap().contains(&selector));
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

        world_remote.match_event(event).unwrap();
        let selector = naming::compute_selector_from_names("ns", "e");
        assert!(world_remote.events.get("ns").unwrap().contains(&selector));
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
            salt: Felt::ONE.into(),
        });

        world_remote.match_event(event).unwrap();
        let selector = naming::compute_selector_from_names("ns", "c");
        assert!(world_remote.contracts.get("ns").unwrap().contains(&selector));
        assert!(world_remote.resources.contains_key(&selector));

        let resource = world_remote.resources.get(&selector).unwrap();
        assert!(matches!(resource, ResourceRemote::Contract(_)));
    }

    #[tokio::test]
    async fn test_model_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "m");

        let resource = ResourceRemote::Model(ModelRemote {
            common: CommonResourceRemoteInfo::new(Felt::ONE, "ns", "m", Felt::ONE),
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::ModelUpgraded(world::ModelUpgraded {
            selector,
            class_hash: Felt::TWO.into(),
            address: Felt::ONE.into(),
            prev_address: Felt::ONE.into(),
        });

        world_remote.match_event(event).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(
            resource.as_model_or_panic().common.class_hashes,
            vec![Felt::ONE.into(), Felt::TWO.into()]
        );
    }

    #[tokio::test]
    async fn test_event_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "e");

        let resource = ResourceRemote::Event(EventRemote {
            common: CommonResourceRemoteInfo::new(Felt::ONE, "ns", "e", Felt::ONE),
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::EventUpgraded(world::EventUpgraded {
            selector,
            class_hash: Felt::TWO.into(),
            address: Felt::ONE.into(),
            prev_address: Felt::ONE.into(),
        });

        world_remote.match_event(event).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(
            resource.as_event_or_panic().common.class_hashes,
            vec![Felt::ONE.into(), Felt::TWO.into()]
        );
    }

    #[tokio::test]
    async fn test_contract_upgraded_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "c");

        let resource = ResourceRemote::Contract(ContractRemote {
            common: CommonResourceRemoteInfo::new(Felt::ONE, "ns", "c", Felt::ONE),
            is_initialized: false,
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::ContractUpgraded(world::ContractUpgraded {
            selector,
            class_hash: Felt::TWO.into(),
        });

        world_remote.match_event(event).unwrap();
        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(
            resource.as_contract_or_panic().common.class_hashes,
            vec![Felt::ONE.into(), Felt::TWO.into()]
        );
    }

    #[tokio::test]
    async fn test_contract_initialized_event() {
        let mut world_remote = WorldRemote::default();
        let selector = naming::compute_selector_from_names("ns", "c");

        let resource = ResourceRemote::Contract(ContractRemote {
            common: CommonResourceRemoteInfo::new(Felt::ONE, "ns", "c", Felt::ONE),
            is_initialized: false,
        });

        world_remote.add_resource(resource);

        let event = WorldEvent::ContractInitialized(world::ContractInitialized {
            selector,
            init_calldata: vec![],
        });

        world_remote.match_event(event).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert!(resource.as_contract_or_panic().initialized);
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

        world_remote.match_event(event).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_namespace_or_panic().writers, HashSet::from([Felt::ONE.into()]));

        let event = WorldEvent::WriterUpdated(world::WriterUpdated {
            resource: selector,
            contract: Felt::ONE.into(),
            value: false,
        });

        world_remote.match_event(event).unwrap();

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

        world_remote.match_event(event).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_namespace_or_panic().owners, HashSet::from([Felt::ONE.into()]));

        let event = WorldEvent::OwnerUpdated(world::OwnerUpdated {
            resource: selector,
            contract: Felt::ONE.into(),
            value: false,
        });

        world_remote.match_event(event).unwrap();

        let resource = world_remote.resources.get(&selector).unwrap();
        assert_eq!(resource.as_namespace_or_panic().owners, HashSet::from([]));
    }
}
