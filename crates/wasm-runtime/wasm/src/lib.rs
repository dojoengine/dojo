use std::sync::Mutex;

use client_blockifier::utils::{addr, invoke_calldata, invoke_tx, HashMap};
use client_blockifier::Client;
use wasm_bindgen::prelude::*;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref CLIENT: Mutex<Client> = Mutex::new(Client::new());
}

fn js_str(s: JsValue) -> String {
    s.as_string().unwrap()
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn test_tx() -> Result<JsValue, String> {
    let res = execute(
        "0x100".into(),
        "0x1".into(),
        "balanceOf".into(),
        serde_wasm_bindgen::to_value(&vec!["0x1", "0x100"]).unwrap(),
    );

    log(&format!("{res:#?}"));
    res
}

#[wasm_bindgen]
pub fn register_class(hash: JsValue, json: JsValue) -> bool {
    let mut client = CLIENT.lock().unwrap();
    let res = client.register_class(&js_str(hash), &js_str(json));
    res.is_ok()
}

#[wasm_bindgen]
pub fn register_class_v0(hash: JsValue, json: JsValue) -> bool {
    let mut client = CLIENT.lock().unwrap();
    let res = client.register_class_v0(&js_str(hash), &js_str(json));
    res.is_ok()
}

#[wasm_bindgen]
pub fn register_contract(address: JsValue, class_hash: JsValue) -> bool {
    let mut client = CLIENT.lock().unwrap();
    client.register_contract(&js_str(address), &js_str(class_hash), HashMap::new()).is_ok()
}

#[wasm_bindgen]
pub fn build_storage_key(storage_var_name: JsValue, args: JsValue) -> String {
    let args: Vec<String> = serde_wasm_bindgen::from_value(args).unwrap();
    let args: Vec<&str> = args.iter().map(|e| e.as_str()).collect();

    format!("{:?}", addr::storage(&js_str(storage_var_name), &args))
}

// #[wasm_bindgen]
// pub fn get_state() -> HashMap<(ContractAddress), StarkFelt> {
//     let mut client = CLIENT.lock().unwrap();
//     client.cache();
// }

#[wasm_bindgen]
pub fn execute(
    caller: JsValue,
    callee: JsValue,
    entrypoint: JsValue,
    calldata: JsValue,
) -> Result<JsValue, String> {
    let caller = js_str(caller);
    let callee = js_str(callee);
    let entrypoint = js_str(entrypoint);

    let calldata: Vec<String> = serde_wasm_bindgen::from_value(calldata).unwrap();
    let calldata = calldata.iter().map(|cd| cd.as_str()).collect();
    let tx = invoke_tx(&caller, invoke_calldata(&callee, &entrypoint, calldata), None, "1");
    let mut client = CLIENT.lock().unwrap();

    if !client.state().contracts.contains_key(&addr::contract(&caller)) {
        client.register_contract(&caller, "0x100", HashMap::new()).unwrap();
    }

    let tx_res = client.execute(tx);
    log(&format!("{tx_res:#?}"));

    match tx_res {
        Ok(exec_info) => {
            let res = exec_info.actual_resources;
            // res.0
            Ok(serde_wasm_bindgen::to_value(&res.0).unwrap())
        }
        Err(tx_err) => Err(format!("{tx_err:#?}")),
    }
}
