//! Fetches the events for the given world address and converts them to remote resources.
//!
//! The world is responsible for managing the remote resources onchain. We are expected
//! to safely unwrap the resources lookup as they are supposed to exist.
//!
//! Events are also sequential, a resource is not expected to be upgraded before
//! being registered. We take advantage of this fact to optimize the data gathering.

use std::collections::HashSet;

use anyhow::Result;
use dojo_types::naming;
use starknet::{
    core::types::{EventFilter, Felt},
    providers::Provider,
};

use super::permissions::PermissionsUpdateable;
use super::{RemoteResource, WorldRemote};
use crate::{
    contracts::abigen::world::{self, Event as WorldEvent},
    remote::{CommonResourceRemoteInfo, ContractRemote, EventRemote, ModelRemote, NamespaceRemote},
};

impl WorldRemote {
    /// Fetch the events from the world and convert them to remote resources.
    pub async fn from_events<P: Provider>(
        &mut self,
        world_address: Felt,
        provider: &P,
    ) -> Result<Self> {
        // We only care about management events, not resource events (set, delete, emit).
        let keys = vec![
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
        ];

        let filter = EventFilter {
            from_block: None,
            to_block: None,
            address: Some(world_address),
            keys: Some(vec![keys]),
        };

        let chunk_size = 500;
        let mut continuation_token = None;

        tracing::trace!(%world_address, ?filter, "Fetching remote world events.");

        let mut events = Vec::new();

        while continuation_token.is_some() {
            let page = provider.get_events(filter.clone(), continuation_token, chunk_size).await?;

            continuation_token = page.continuation_token;
            events.extend(page.events);
        }

        // TODO: move this logic into a function to ease the testing without having to mock the
        // event fetching.
        for event in events {
            match world::Event::try_from(event) {
                Ok(ev) => {
                    tracing::trace!(?ev, "Processing world event.");

                    match ev {
                        WorldEvent::WorldSpawned(e) => {
                            self.class_hashes.push(e.class_hash.into());
                        }
                        WorldEvent::WorldUpgraded(e) => {
                            self.class_hashes.push(e.class_hash.into());
                        }
                        WorldEvent::NamespaceRegistered(e) => {
                            self.namespaces.insert(e.hash);

                            self.resources.insert(
                                e.hash,
                                RemoteResource::Namespace(NamespaceRemote::new(
                                    e.namespace.to_string()?,
                                )),
                            );
                        }
                        WorldEvent::ModelRegistered(e) => {
                            let model_remote = ModelRemote {
                                common: CommonResourceRemoteInfo::new(
                                    e.class_hash.into(),
                                    e.name.to_string()?,
                                    e.address.into(),
                                ),
                            };

                            let namespace = e.namespace.to_string()?;
                            let dojo_selector = naming::compute_selector_from_names(
                                &namespace,
                                &e.name.to_string()?,
                            );

                            self.models
                                .entry(namespace)
                                .or_insert_with(HashSet::new)
                                .insert(dojo_selector);

                            self.resources
                                .insert(dojo_selector, RemoteResource::Model(model_remote));
                        }
                        WorldEvent::EventRegistered(e) => {
                            let event_remote = EventRemote {
                                common: CommonResourceRemoteInfo::new(
                                    e.class_hash.into(),
                                    e.name.to_string()?,
                                    e.address.into(),
                                ),
                            };

                            let namespace = e.namespace.to_string()?;
                            let dojo_selector = naming::compute_selector_from_names(
                                &namespace,
                                &e.name.to_string()?,
                            );

                            self.events
                                .entry(namespace)
                                .or_insert_with(HashSet::new)
                                .insert(dojo_selector);

                            self.resources
                                .insert(dojo_selector, RemoteResource::Event(event_remote));
                        }
                        WorldEvent::ContractRegistered(e) => {
                            let contract_remote = ContractRemote {
                                common: CommonResourceRemoteInfo::new(
                                    e.class_hash.into(),
                                    e.name.to_string()?,
                                    e.address.into(),
                                ),
                                initialized: false,
                            };

                            let namespace = e.namespace.to_string()?;
                            let dojo_selector = naming::compute_selector_from_names(
                                &namespace,
                                &e.name.to_string()?,
                            );

                            self.contracts
                                .entry(namespace)
                                .or_insert_with(HashSet::new)
                                .insert(dojo_selector);

                            self.resources
                                .insert(dojo_selector, RemoteResource::Contract(contract_remote));
                        }
                        WorldEvent::ModelUpgraded(e) => {
                            // Unwrap is safe because the model must exist in the world.
                            let resource = self.resources.get_mut(&e.selector).unwrap();
                            resource.push_class_hash(e.class_hash.into());
                        }
                        WorldEvent::EventUpgraded(e) => {
                            // Unwrap is safe because the event must exist in the world.
                            let resource = self.resources.get_mut(&e.selector).unwrap();
                            resource.push_class_hash(e.class_hash.into());
                        }
                        WorldEvent::ContractUpgraded(e) => {
                            // Unwrap is safe because the contract must exist in the world.
                            let resource = self.resources.get_mut(&e.selector).unwrap();
                            resource.push_class_hash(e.class_hash.into());
                        }
                        WorldEvent::ContractInitialized(e) => {
                            // Unwrap is safe bcause the contract must exist in the world.
                            let resource = self.resources.get_mut(&e.selector).unwrap();
                            let contract = resource.as_contract_mut()?;
                            contract.initialized = true;
                        }
                        WorldEvent::WriterUpdated(e) => {
                            // Unwrap is safe because the resource must exist in the world.
                            let resource = self.resources.get_mut(&e.resource).unwrap();
                            resource.update_writer(e.contract.into(), e.value)?;
                        }
                        WorldEvent::OwnerUpdated(e) => {
                            // Unwrap is safe because the resource must exist in the world.
                            let resource = self.resources.get_mut(&e.resource).unwrap();
                            resource.update_owner(e.contract.into(), e.value)?;
                        }
                        _ => {
                            // Ignore events filtered out by the event filter.
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        ?e,
                        "Failed to parse remote world event which is supposed to be valid."
                    );
                }
            }
        }

        Ok(Self::default())
    }
}
