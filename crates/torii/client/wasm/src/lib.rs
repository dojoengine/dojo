//! Minimal JS bindings for the torii client.

use std::str::FromStr;

use futures::StreamExt;
use starknet::core::types::FieldElement;
use starknet::core::utils::cairo_short_string_to_felt;
use wasm_bindgen::prelude::*;

mod utils;

use utils::parse_ty_as_json_str;

type JsFieldElement = JsValue;
type JsEntityModel = JsValue;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub struct Client {
    inner: torii_client::client::Client,
}

#[wasm_bindgen]
impl Client {
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

    /// Returns the list of entities that are currently being synced.
    #[wasm_bindgen(getter, js_name = syncedEntities)]
    pub fn synced_entities(&self) -> Result<JsValue, JsValue> {
        #[cfg(feature = "console-error-panic")]
        console_error_panic_hook::set_once();

        let entities = self.inner.synced_entities();
        serde_wasm_bindgen::to_value(&entities).map_err(|e| e.into())
    }

    #[wasm_bindgen(js_name = addEntitiesToSync)]
    pub async fn add_entities_to_sync(
        &mut self,
        entities: Vec<JsEntityModel>,
    ) -> Result<(), JsValue> {
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

    #[wasm_bindgen(js_name = onEntityChange)]
    pub fn on_entity_change(&self, entity: JsEntityModel) -> Result<(), JsValue> {
        let entity = serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>(entity)?;
        let model = cairo_short_string_to_felt(&entity.model).expect("invalid model name");
        let mut rcv = self.inner.storage().add_listener(model, &entity.keys).unwrap();

        log(&format!("listening to {}", entity.model));

        wasm_bindgen_futures::spawn_local(async move {
            while let Some(event) = rcv.next().await {
                log(&format!("received event for {}: {:?}", entity.model, event));
            }
        });

        Ok(())
    }
}

/// Spawns the client along with the subscription service.
#[wasm_bindgen]
pub async fn spawn_client(
    torii_url: &str,
    rpc_url: &str,
    world_address: &str,
    initial_entities_to_sync: Vec<JsEntityModel>,
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

    Ok(Client { inner: client })
}
