use std::sync::Arc;

use async_std::sync::RwLock as AsyncRwLock;
use starknet::core::types::FieldElement;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use torii_client::provider::jsonrpc::JsonRpcProvider;
use torii_client::storage::EntityStorage;
use torii_client::sync::{self, Client, Entity};
use url::Url;
use wasm_bindgen::prelude::*;

mod storage;

use storage::InMemoryStorage;

/// A type wrapper to expose the client to WASM.
#[wasm_bindgen]
pub struct WasmClient(Client<InMemoryStorage, JsonRpcProvider<HttpTransport>>);

#[wasm_bindgen]
impl WasmClient {
    /// Create an instance of the client. This will create an instance of the client
    /// without any entity models to sync.
    ///
    /// # Arguments
    /// * `url` - The url of the Starknet JSON-RPC provider.
    /// * `world_address` - The address of the World contract to sync with.
    #[wasm_bindgen(constructor)]
    pub fn new(url: &str, world_address: &str) -> Self {
        let world_address = FieldElement::from_hex_be(world_address).unwrap();

        let storage = Arc::new(AsyncRwLock::new(InMemoryStorage::new()));
        let provider = JsonRpcProvider::new(
            JsonRpcClient::new(HttpTransport::new(Url::parse(url).unwrap())),
            world_address,
        );

        Self(sync::Client::new(storage, provider, vec![]))
    }

    /// Start the syncing loop.
    pub async fn start(&self) -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        self.0.start().await.map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Returns the model values of the requested entity keys.
    #[wasm_bindgen(js_name = getModelValue)]
    pub async fn get_model_value(
        &self,
        model: &str,
        keys: JsValue,
        length: usize,
    ) -> Result<JsValue, JsValue> {
        console_error_panic_hook::set_once();

        let keys = serde_wasm_bindgen::from_value::<Vec<FieldElement>>(keys)?;
        let values = self
            .0
            .storage()
            .read()
            .await
            .get(
                cairo_short_string_to_felt(model)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?,
                keys,
                length,
            )
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(serde_wasm_bindgen::to_value(&values)?)
    }

    /// Add a new entity to be synced by the client.
    #[wasm_bindgen(js_name = addEntityToSync)]
    pub fn add_entity_to_sync(&self, entity: JsValue) -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        let entity = serde_wasm_bindgen::from_value::<Entity>(entity)?;
        self.0.sync_entities.write().insert(entity);
        Ok(())
    }

    /// Returns the list of entities that are currently being synced.
    #[wasm_bindgen(getter, js_name = syncedEntities)]
    pub fn synced_entities(&self) -> Result<JsValue, JsValue> {
        console_error_panic_hook::set_once();
        Ok(serde_wasm_bindgen::to_value(&self.0.sync_entities.read().iter().collect::<Vec<_>>())?)
    }
}
