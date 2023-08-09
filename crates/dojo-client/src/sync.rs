use std::sync::Arc;
use std::time::Duration;

use async_std::stream::{self, Interval, StreamExt};
use async_std::sync::RwLock;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet_crypto::FieldElement;
use thiserror::Error;

use crate::provider::Provider;
use crate::storage::EntityStorage;

#[derive(Debug, Error)]
pub enum SyncerError<S, P> {
    #[error(transparent)]
    Provider(P),
    #[error(transparent)]
    Storage(S),
}

/// Request to sync a component of an entity.
pub struct EntityComponentReq {
    /// Component name
    pub component: String,
    /// The entity keys
    pub keys: Vec<FieldElement>,
}

pub struct WorldPartialSyncer<S: EntityStorage, P: Provider> {
    // We wrap it in an Arc<RwLock> to allow sharing the storage between threads.
    /// Storage to store the synced entity component values.
    storage: Arc<RwLock<S>>,
    /// A provider implementation to query the World for entity components.
    provider: P,
    /// The entity components to sync.
    entity_components_to_sync: Vec<EntityComponentReq>,
    /// The interval to run the syncing loop.
    interval: Interval,
}

impl<S, P> WorldPartialSyncer<S, P>
where
    S: EntityStorage + Send + Sync,
    P: Provider + Sync + 'static,
{
    const DEFAULT_INTERVAL: Duration = Duration::from_secs(1);

    pub fn new(
        storage: Arc<RwLock<S>>,
        provider: P,
        entities: Vec<EntityComponentReq>,
    ) -> WorldPartialSyncer<S, P> {
        Self {
            storage,
            provider,
            entity_components_to_sync: entities,
            interval: stream::interval(Self::DEFAULT_INTERVAL),
        }
    }

    pub fn with_interval(mut self, milisecond: u64) -> Self {
        let duration = Duration::from_millis(milisecond);
        self.interval = stream::interval(duration);
        self
    }

    /// Starts the syncing process.
    /// This function will run forever.
    pub async fn start(&mut self) -> Result<(), SyncerError<S::Error, P::Error>> {
        while self.interval.next().await.is_some() {
            for entity in &self.entity_components_to_sync {
                let values = self
                    .provider
                    .entity(&entity.component, entity.keys.clone())
                    .await
                    .map_err(SyncerError::Provider)?;

                self.storage
                    .write()
                    .await
                    .set(
                        cairo_short_string_to_felt(&entity.component).unwrap(),
                        entity.keys.clone(),
                        values,
                    )
                    .await
                    .map_err(SyncerError::Storage)?
            }
        }

        Ok(())
    }
}
