use std::sync::Arc;
use std::time::Duration;

use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::Provider;
use starknet_crypto::FieldElement;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::{Instant, Interval};

use crate::contract::component::{ComponentError, ComponentReader};
use crate::contract::world::WorldContractReader;
use crate::storage::EntityStorage;

#[derive(Debug, Error)]
pub enum SyncerError<S, P> {
    #[error(transparent)]
    Component(ComponentError<P>),
    #[error(transparent)]
    Storage(S),
}

/// Request to sync a component of an entity.
/// This struct will be used to construct an [EntityReader].
pub struct EntityComponentReq {
    /// Component name
    pub component: String,
    /// The entity keys
    pub keys: Vec<FieldElement>,
}

/// A type which wraps a [ComponentReader] for reading a component value of an entity.
pub struct EntityReader<'a, P: Provider + Sync> {
    /// Component name
    component: String,
    /// The entity keys
    keys: Vec<FieldElement>,
    /// Component reader
    reader: ComponentReader<'a, P>,
}

pub struct WorldPartialSyncer<'a, S: EntityStorage, P: Provider + Sync> {
    // We wrap it in an Arc<RwLock> to allow sharing the storage between threads.
    /// Storage to store the synced entity component values.
    storage: Arc<RwLock<S>>,
    /// Client for reading the World contract.
    world_reader: &'a WorldContractReader<'a, P>,
    /// The entity components to sync.
    entity_components_to_sync: Vec<EntityComponentReq>,
    /// The interval to run the syncing loop.
    interval: Interval,
}

impl<'a, S, P> WorldPartialSyncer<'a, S, P>
where
    S: EntityStorage + Send + Sync,
    P: Provider + Sync + 'static,
{
    const DEFAULT_INTERVAL: Duration = Duration::from_secs(1);

    pub fn new(
        storage: Arc<RwLock<S>>,
        world_reader: &'a WorldContractReader<'a, P>,
        entities: Vec<EntityComponentReq>,
    ) -> WorldPartialSyncer<'a, S, P> {
        Self {
            world_reader,
            storage,
            entity_components_to_sync: entities,
            interval: tokio::time::interval_at(
                Instant::now() + Self::DEFAULT_INTERVAL,
                Self::DEFAULT_INTERVAL,
            ),
        }
    }

    pub fn with_interval(mut self, milisecond: u64) -> Self {
        let interval = Duration::from_millis(milisecond);
        self.interval = tokio::time::interval_at(Instant::now() + interval, interval);
        self
    }

    /// Starts the syncing process.
    /// This function will run forever.
    pub async fn start(&mut self) -> Result<(), SyncerError<S::Error, P::Error>> {
        let entity_readers = self.entity_readers().await?;

        loop {
            self.interval.tick().await;

            for reader in &entity_readers {
                let values = reader
                    .reader
                    .entity(reader.keys.clone(), BlockId::Tag(BlockTag::Pending))
                    .await
                    .map_err(SyncerError::Component)?;

                self.storage
                    .write()
                    .await
                    .set(
                        cairo_short_string_to_felt(&reader.component).unwrap(),
                        reader.keys.clone(),
                        values,
                    )
                    .await
                    .map_err(SyncerError::Storage)?
            }
        }
    }

    /// Get the entity reader for every requested component to sync.
    async fn entity_readers(
        &self,
    ) -> Result<Vec<EntityReader<'a, P>>, SyncerError<S::Error, P::Error>> {
        let mut entity_readers = Vec::new();

        for i in &self.entity_components_to_sync {
            let comp_reader = self
                .world_reader
                .component(&i.component, BlockId::Tag(BlockTag::Pending))
                .await
                .map_err(SyncerError::Component)?;

            entity_readers.push(EntityReader {
                component: i.component.clone(),
                keys: i.keys.clone(),
                reader: comp_reader,
            });
        }

        Ok(entity_readers)
    }
}
