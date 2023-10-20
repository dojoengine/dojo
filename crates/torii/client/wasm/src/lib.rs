//! Minimal JS bindings for the torii client.

use std::str::FromStr;

use futures::StreamExt;
use starknet::core::types::FieldElement;
use starknet::core::utils::cairo_short_string_to_felt;
use types::ClientConfig;
use wasm_bindgen::prelude::*;

mod types;
mod utils;

use utils::parse_ty_as_json_str;

type JsFieldElement = JsValue;
type JsEntityModel = JsValue;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

#[wasm_bindgen]
pub struct Client {
    inner: torii_client::client::Client,
}

#[wasm_bindgen]
impl Client {
    /// Retrieves the model value of an entity.
    #[wasm_bindgen(js_name = getModelValue)]
    pub fn get_model_value(
        &self,
        model: &str,
        keys: Vec<JsFieldElement>,
    ) -> Result<JsValue, JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let keys = keys
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<FieldElement>)
            .collect::<Result<Vec<FieldElement>, _>>()
            .map_err(|err| {
                JsValue::from_str(format!("failed to parse entity keys: {err}").as_str())
            })?;

        match self.inner.entity(model, &keys) {
            Some(ty) => Ok(serde_wasm_bindgen::to_value(&parse_ty_as_json_str(&ty))?),
            None => Ok(JsValue::NULL),
        }
    }

    /// Register new entities to be synced.
    #[wasm_bindgen(js_name = addEntitiesToSync)]
    pub async fn add_entities_to_sync(&self, entities: Vec<JsEntityModel>) -> Result<(), JsValue> {
        log("adding entities to sync...");

        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let entities = entities
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>)
            .collect::<Result<Vec<_>, _>>()?;

        self.inner
            .add_entities_to_sync(entities)
            .await
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Remove the entities from being synced.
    #[wasm_bindgen(js_name = removeEntitiesToSync)]
    pub async fn remove_entities_to_sync(
        &self,
        entities: Vec<JsEntityModel>,
    ) -> Result<(), JsValue> {
        log("removing entities to sync...");

        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let entities = entities
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>)
            .collect::<Result<Vec<_>, _>>()?;

        self.inner
            .remove_entities_to_sync(entities)
            .await
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    /// Register a callback to be called every time the specified entity change.
    #[wasm_bindgen(js_name = onEntityChange)]
    pub fn on_entity_change(
        &self,
        entity: JsEntityModel,
        callback: js_sys::Function,
    ) -> Result<(), JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let entity = serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>(entity)?;
        let model = cairo_short_string_to_felt(&entity.model).expect("invalid model name");
        let mut rcv = self.inner.storage().add_listener(model, &entity.keys).unwrap();

        wasm_bindgen_futures::spawn_local(async move {
            while rcv.next().await.is_some() {
                let _ = callback.call0(&JsValue::null());
            }
        });

        Ok(())
    }
}

/// Create the a client with the given configurations.
#[wasm_bindgen(js_name = createClient)]
pub async fn create_client(
    initial_entities_to_sync: Vec<JsEntityModel>,
    config: ClientConfig,
) -> Result<Client, JsValue> {
    #[cfg(feature = "console-error-panic")]
    console_error_panic_hook::set_once();

    let ClientConfig { rpc_url, torii_url, world_address } = config;

    let entities = initial_entities_to_sync
        .into_iter()
        .map(serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>)
        .collect::<Result<Vec<_>, _>>()?;

    let world_address = FieldElement::from_str(&world_address).map_err(|err| {
        JsValue::from_str(format!("failed to parse world address: {err}").as_str())
    })?;

    let client = torii_client::client::ClientBuilder::new()
        .set_entities_to_sync(entities)
        .build(torii_url, rpc_url, world_address)
        .await
        .map_err(|err| JsValue::from_str(format!("failed to build client: {err}").as_str()))?;

    wasm_bindgen_futures::spawn_local(client.start_subscription().await.map_err(|err| {
        JsValue::from_str(
            format!("failed to start torii client subscription service: {err}").as_str(),
        )
    })?);

    Ok(Client { inner: client })
}
