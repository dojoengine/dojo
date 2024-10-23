//! Fetches the events for the given world address and converts them to remote resources.
//!

use anyhow::Result;
use starknet::{
    core::types::{EventFilter, Felt},
    providers::Provider,
};

use super::WorldRemote;
use crate::contracts::abigen::world::{self, Event as WorldEvent};

impl WorldRemote {
    pub async fn from_events<P: Provider>(&mut self, world_address: Felt, provider: &P) -> Result<Self> {
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
            let page =
                provider.get_events(filter.clone(), continuation_token, chunk_size).await?;

            continuation_token = page.continuation_token;
            events.extend(page.events);
        }

        for event in events {
            match world::Event::try_from(event) {
                Ok(ev) => {
                    tracing::trace!(?ev, "Processing world event.");

                    match ev {
                        WorldEvent::WorldSpawned(e) => {
                            self.original_class_hash = e.class_hash.into();
                        },
                        WorldEvent::WorldUpgraded(e) => {
                            self.current_class_hash = e.class_hash.into();
                        },
                        WorldEvent::NamespaceRegistered(e) => {
                            self.namespaces.push(e.namespace.to_string()?);
                        },
                        WorldEvent::ModelRegistered(e) => {
                            let model_name = e.name.to_string()?;
                            let namespace = e.namespace.to_string()?;
                        },
                        WorldEvent::EventRegistered(e) => {
                            
                        },
                        WorldEvent::ContractRegistered(e) => {
                            
                        },
                        WorldEvent::ModelUpgraded(e) => {
                            
                        },
                        WorldEvent::EventUpgraded(e) => {
                            
                        },
                        WorldEvent::ContractUpgraded(e) => {
                            
                        },
                        WorldEvent::ContractInitialized(e) => {
                            
                        },
                        WorldEvent::WriterUpdated(e) => {
                            
                        },
                        WorldEvent::OwnerUpdated(e) => {
                            
                        },
                        _ => {
                            // Ignore events filtered out by the event filter.
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(?e, "Failed to parse remote world event which is supposed to be valid.");
                }
            }
        }

        Ok(Self::default())
    }
}
