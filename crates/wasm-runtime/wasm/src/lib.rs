mod utils;
use std::sync::Mutex;

use client_sequencer::utils::{addr, invoke_calldata, invoke_tx, HashMap};
use client_sequencer::Client;
use utils::set_panic_hook;
use wasm_bindgen::prelude::*;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref CLIENT: Mutex<Client> = Mutex::new(Client::new());
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn debug() {
    set_panic_hook();
    log("Panic hook set");
}

#[wasm_bindgen]
pub fn test_tx() -> JsValue {
    let res = execute(
        "0x100".into(),
        "0x1".into(),
        "balanceOf".into(),
        serde_wasm_bindgen::to_value(&vec!["0x1", "0x100"]).unwrap(),
    );

    res
}

#[wasm_bindgen]
pub fn register_class(hash: String, json: String) -> bool {
    let mut client = CLIENT.lock().unwrap();
    match client.register_class(&hash, &json) {
        Ok(_) => true,
        Err(e) => {
            log(&format!("{:?}", e));
            false
        }
    }
}

#[wasm_bindgen]
pub fn register_class_v0(hash: String, json: String) -> bool {
    let mut client = CLIENT.lock().unwrap();
    match client.register_class_v0(&hash, &json) {
        Ok(_) => true,
        Err(e) => {
            log(&format!("{:?}", e));
            false
        }
    }
}

#[wasm_bindgen]
pub fn register_contract(address: String, class_hash: String) -> bool {
    let mut client = CLIENT.lock().unwrap();
    match client.register_contract(&address, &class_hash, HashMap::new()) {
        Ok(_) => true,
        Err(e) => {
            log(&format!("{:?}", e));
            false
        }
    }
}

#[wasm_bindgen]
pub fn build_storage_key(storage_var_name: String, args: JsValue) -> JsValue {
    let args: Vec<String> = match serde_wasm_bindgen::from_value(args) {
        Ok(val) => val,
        Err(e) => {
            log(&format!("Err: {}", e));
            return JsValue::FALSE;
        }
    };
    let args: Vec<&str> = args.iter().map(|e| e.as_str()).collect();
    log(&format!("{}, Args: {:?}", storage_var_name, args));
    JsValue::from_str(&format!("{:?}", addr::storage(&storage_var_name, &args)))
}

// #[wasm_bindgen]
// pub fn get_state() -> HashMap<(ContractAddress), StarkFelt> {
//     let mut client = CLIENT.lock().unwrap();
//     client.cache();
// }

#[wasm_bindgen]
pub fn execute(caller: String, callee: String, entrypoint: String, calldata: JsValue) -> JsValue {
    let calldata: Vec<String> = serde_wasm_bindgen::from_value(calldata).unwrap();
    let calldata = calldata.iter().map(|cd| cd.as_str()).collect();

    log(&format!(
        "caller: {} callee: {} entrypoint: {} \n\n{:?}",
        &caller, &callee, &entrypoint, calldata
    ));

    let tx = invoke_tx(&caller, invoke_calldata(&callee, &entrypoint, calldata), None, "1");
    let mut client = CLIENT.lock().unwrap();

    if !client.state().contracts.contains_key(&addr::contract(&caller)) {
        client.register_contract(&caller, "0x100", HashMap::new()).unwrap();
    }

    let tx_res = client.execute(tx);

    match tx_res {
        Ok(exec_info) => {
            log(&format!("execute_call_info: {:#?}", exec_info.execute_call_info));
            log(&format!("validate_call_info: {:#?}", exec_info.validate_call_info));

            serde_wasm_bindgen::to_value(&vec![2, 5, 7, 9, 11]).unwrap()
        }
        Err(tx_err) => {
            log(&format!("{:#?}", tx_err));
            JsValue::FALSE
        }
    }
}
