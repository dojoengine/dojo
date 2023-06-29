use async_trait::async_trait;
use starknet::providers::jsonrpc::{JsonRpcClientError, JsonRpcTransport};
use starknet::providers::{JsonRpcClient, ProviderError};

use super::Provider;

pub struct JsonRpcProvider<T> {
    client: JsonRpcClient<T>,
}

impl<T> JsonRpcProvider<T> {
    pub fn new(client: JsonRpcClient<T>) -> Self {
        Self { client }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T> Provider for JsonRpcProvider<T>
where
    T: JsonRpcTransport,
{
    type Error = ProviderError<JsonRpcClientError<T::Error>>;
}
