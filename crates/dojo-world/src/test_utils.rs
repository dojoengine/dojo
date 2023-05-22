use std::collections::HashMap;
use std::fmt::Display;

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use starknet::providers::jsonrpc::{JsonRpcMethod, JsonRpcResponse, JsonRpcTransport};
use thiserror::Error;

pub struct MockJsonRpcTransport {
    responses: HashMap<(String, String), String>,
}

impl MockJsonRpcTransport {
    pub fn new() -> Self {
        MockJsonRpcTransport { responses: HashMap::new() }
    }

    pub fn set_response(&mut self, method: JsonRpcMethod, params: Value, response: Value) {
        let method = serde_json::to_string(&method).unwrap();
        let params = serde_json::to_string(&params).unwrap();
        let response = serde_json::to_string(&response).unwrap();
        self.responses.insert((method, params), response);
    }
}

#[derive(Debug, Error)]
pub struct MockError {
    msg: String,
}

impl Display for MockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

#[async_trait]
impl JsonRpcTransport for MockJsonRpcTransport {
    type Error = MockError;

    async fn send_request<P, R>(
        &self,
        method: JsonRpcMethod,
        params: P,
    ) -> Result<JsonRpcResponse<R>, Self::Error>
    where
        P: Serialize + Send,
        R: DeserializeOwned,
    {
        let method = serde_json::to_string(&method).unwrap();
        let params = serde_json::to_string(&params).unwrap();
        match self.responses.get(&(method.clone(), params.clone())) {
            Some(res) => serde_json::from_str(res).map_err(|e| MockError { msg: e.to_string() }),
            None => {
                panic!("Response not set in mock for method {method:?} and params {params:?}")
            }
        }
    }
}
