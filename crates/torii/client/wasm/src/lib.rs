//! Minimal JS bindings for the torii client.

use std::str::FromStr;

use starknet::core::types::FieldElement;
use wasm_bindgen::prelude::*;

mod utils;

use utils::parse_ty_as_json_str;

type JsFieldElement = JsValue;
type JsEntityComponent = JsValue;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub struct Client(torii_client::client::Client);

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(js_name = getModelValue)]
    pub async fn get_model_value(
        &self,
        model: &str,
        keys: Vec<JsFieldElement>,
    ) -> Result<Option<JsValue>, JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let keys = keys
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<FieldElement>)
            .collect::<Result<Vec<FieldElement>, _>>()
            .map_err(|err| {
                JsValue::from_str(format!("failed to parse entity keys: {err}").as_str())
            })?;

        match self.0.entity(model, &keys) {
            Some(ty) => {
                let json = parse_ty_as_json_str(&ty);
                Ok(Some(serde_wasm_bindgen::to_value(&json)?))
            }
            None => Ok(None),
        }
    }

    /// Returns the list of entities that are currently being synced.
    #[wasm_bindgen(getter, js_name = syncedEntities)]
    pub fn synced_entities(&self) -> Result<JsValue, JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let entities = self.0.synced_entities();
        serde_wasm_bindgen::to_value(&entities).map_err(|e| e.into())
    }

    #[wasm_bindgen(js_name = addEntitiesToSync)]
    pub async fn add_entities_to_sync(
        &mut self,
        entities: Vec<JsEntityComponent>,
    ) -> Result<(), JsValue> {
        log("adding entities to sync...");

        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let entities = entities
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>)
            .collect::<Result<Vec<_>, _>>()?;

        self.0
            .add_entities_to_sync(entities)
            .await
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }

    #[wasm_bindgen(js_name = removeEntitiesToSync)]
    pub async fn remove_entities_to_sync(
        &self,
        entities: Vec<JsEntityComponent>,
    ) -> Result<(), JsValue> {
        log("removing entities to sync...");

        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let entities = entities
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>)
            .collect::<Result<Vec<_>, _>>()?;

        self.0
            .remove_entities_to_sync(entities)
            .await
            .map_err(|err| JsValue::from_str(&err.to_string()))
    }
}

/// Spawns the client along with the subscription service.
#[wasm_bindgen]
pub async fn spawn_client(
    torii_url: &str,
    rpc_url: &str,
    world_address: &str,
    initial_entities_to_sync: Vec<JsEntityComponent>,
) -> Result<Client, JsValue> {
    #[cfg(feature = "console-error-panic")]
    console_error_panic_hook::set_once();

    let entities = initial_entities_to_sync
        .into_iter()
        .map(serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>)
        .collect::<Result<Vec<_>, _>>()?;

    let world_address = FieldElement::from_str(world_address).map_err(|err| {
        JsValue::from_str(format!("failed to parse world address: {err}").as_str())
    })?;

    let client = torii_client::client::ClientBuilder::new()
        .set_entities_to_sync(entities)
        .build(torii_url.into(), rpc_url.into(), world_address)
        .await
        .map_err(|err| JsValue::from_str(format!("failed to build client: {err}").as_str()))?;

    wasm_bindgen_futures::spawn_local(client.start_subscription().await.map_err(|err| {
        JsValue::from_str(
            format!("failed to start torii client subscription service: {err}").as_str(),
        )
    })?);

    Ok(Client(client))
}
