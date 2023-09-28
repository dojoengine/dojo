use std::collections::HashSet;
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use async_std::sync::RwLock as AsyncRwLock;
use parking_lot::RwLock;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet_crypto::FieldElement;
use thiserror::Error;
#[cfg(not(target_arch = "wasm32"))]
use tokio::{sync::RwLock as AsyncRwLock, time::Duration};
#[cfg(target_arch = "wasm32")]
use web_sys::WorkerGlobalScope;

use crate::provider::Provider;
use crate::storage::EntityStorage;

#[derive(Debug, Error)]
pub enum ClientError<S, P> {
    #[error(transparent)]
    Provider(P),
    #[error(transparent)]
    Storage(S),
}

/// Request to sync a model of an entity.
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Entity {
    /// Model name
    pub model: String,
    /// The entity keys
    pub keys: Vec<FieldElement>,
}

/// A client to sync entity models from the World.
pub struct Client<S, P> {
    // We wrap it in an Arc<RwLock> to allow sharing the storage between threads.
    /// Storage to store the synced entity model values.
    storage: Arc<AsyncRwLock<S>>,
    /// A provider implementation to query the World for entity models.
    provider: P,
    /// The entity models to sync.
    pub sync_entities: RwLock<HashSet<Entity>>,

    /// The interval to run the syncing loop.
    // DEV: It's wrapped in [parking_lot::RwLock] to prevent from having the `Self::start()` to be
    // `mut`. Having it on `mut` seems to be causing this issue https://github.com/rustwasm/wasm-bindgen/issues/1578 when compiled to wasm32.
    #[cfg(not(target_arch = "wasm32"))]
    interval: AsyncRwLock<tokio::time::Interval>,
    #[cfg(target_arch = "wasm32")]
    interval: i32,
}

impl<S, P> Client<S, P>
where
    S: EntityStorage + Send + Sync,
    P: Provider + Send + Sync + 'static,
{
    #[cfg(not(target_arch = "wasm32"))]
    const DEFAULT_INTERVAL: Duration = Duration::from_secs(1);

    #[cfg(target_arch = "wasm32")]
    const DEFAULT_INTERVAL: i32 = 1000; // 1 second

    pub fn new(storage: Arc<AsyncRwLock<S>>, provider: P, entities: Vec<Entity>) -> Client<S, P> {
        Self {
            storage,
            provider,
            sync_entities: RwLock::new(HashSet::from_iter(entities)),
            #[cfg(not(target_arch = "wasm32"))]
            interval: AsyncRwLock::new(tokio::time::interval_at(
                tokio::time::Instant::now() + Self::DEFAULT_INTERVAL,
                Self::DEFAULT_INTERVAL,
            )),
            #[cfg(target_arch = "wasm32")]
            interval: Self::DEFAULT_INTERVAL,
        }
    }

    pub fn with_interval(
        mut self,
        #[cfg(not(target_arch = "wasm32"))] milisecond: u64,
        #[cfg(target_arch = "wasm32")] milisecond: i32,
    ) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use tokio::time::{interval_at, Instant};
            let interval = Duration::from_millis(milisecond);
            self.interval = AsyncRwLock::new(interval_at(Instant::now() + interval, interval));
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.interval = milisecond;
        }

        self
    }

    /// Returns the storage instance used by the client.
    pub fn storage(&self) -> Arc<AsyncRwLock<S>> {
        self.storage.clone()
    }

    /// Starts the syncing process.
    /// This function will run forever.
    pub async fn start(&self) -> Result<(), ClientError<S::Error, P::Error>> {
        loop {
            #[cfg(not(target_arch = "wasm32"))]
            self.interval.write().await.tick().await;

            #[cfg(target_arch = "wasm32")]
            sleep(self.interval).await;

            let entities = self.sync_entities.read().clone();
            for entity in entities {
                let values = self
                    .provider
                    .entity(&entity.model, entity.keys.clone())
                    .await
                    .map_err(ClientError::Provider)?;

                #[cfg(not(target_arch = "wasm32"))]
                self.storage
                    .write()
                    .await
                    .set(cairo_short_string_to_felt(&entity.model).unwrap(), entity.keys, values)
                    .await
                    .map_err(ClientError::Storage)?;

                #[cfg(target_arch = "wasm32")]
                self.storage
                    .write()
                    .await
                    .set(cairo_short_string_to_felt(&entity.model).unwrap(), entity.keys, values)
                    .await
                    .map_err(ClientError::Storage)?;
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn sleep(delay: i32) {
    use wasm_bindgen::JsCast;
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        // if we are in a worker, use the global worker's scope
        // otherwise, use the window's scope
        if let Ok(worker_scope) = js_sys::global().dyn_into::<WorkerGlobalScope>() {
            worker_scope
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay)
                .expect("should register `setTimeout`");
        } else {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay)
                .expect("should register `setTimeout`");
        }
    };
    let p = js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
}
