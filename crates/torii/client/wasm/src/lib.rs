use std::str::FromStr;

use starknet::core::types::FieldElement;
use wasm_bindgen::prelude::*;

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
    /// Returns the model values of the requested entity.
    #[wasm_bindgen(js_name = getModelValue)]
    pub async fn get_model_value(
        &self,
        model: &str,
        keys: Vec<JsFieldElement>,
    ) -> Result<Option<JsValue>, JsValue> {
        console_error_panic_hook::set_once();

        let keys = keys
            .into_iter()
            .map(serde_wasm_bindgen::from_value::<FieldElement>)
            .collect::<Result<Vec<FieldElement>, _>>()
            .map_err(|err| {
                JsValue::from_str(format!("failed to parse entity keys: {err}").as_str())
            })?;

        match self.0.entity(model, &keys) {
            Some(values) => Ok(Some(serde_wasm_bindgen::to_value(&values)?)),
            None => Ok(None),
        }
    }

    /// Returns the list of entities that are currently being synced.
    #[wasm_bindgen(getter, js_name = syncedEntities)]
    pub fn synced_entities(&self) -> Result<JsValue, JsValue> {
        console_error_panic_hook::set_once();
        let entities = self.0.synced_entities();
        serde_wasm_bindgen::to_value(&entities).map_err(|e| e.into())
    }
}

#[wasm_bindgen]
pub async fn spawn_client(
    torii_url: &str,
    rpc_url: &str,
    world_address: &str,
    initial_entities_to_sync: Vec<JsEntityComponent>,
) -> Result<Client, JsValue> {
    console_error_panic_hook::set_once();

    let entities = initial_entities_to_sync
        .into_iter()
        .map(serde_wasm_bindgen::from_value::<dojo_types::schema::EntityModel>)
        .collect::<Result<Vec<_>, _>>()?;

    let world_address = FieldElement::from_str(world_address).map_err(|err| {
        JsValue::from_str(format!("failed to parse World address: {err}").as_str())
    })?;

    let client = torii_client::client::ClientBuilder::new()
        .set_entities_to_sync(entities)
        .build(torii_url.into(), rpc_url.into(), world_address)
        .await
        .map_err(|err| JsValue::from_str(format!("failed to build client: {err}").as_str()))?;

    Ok(Client(client))
}
