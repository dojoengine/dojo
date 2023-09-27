use async_trait::async_trait;
use starknet::core::types::{BlockId, BlockTag};
use starknet::providers::jsonrpc::{JsonRpcClientError, JsonRpcTransport};
use starknet::providers::JsonRpcClient;
use starknet_crypto::FieldElement;

use super::Provider;
use crate::contract::component::ComponentError;
use crate::contract::world::WorldContractReader;

#[derive(Debug, thiserror::Error)]
pub enum JsonRpcProviderError<P> {
    #[error(transparent)]
    ComponetReader(ComponentError<P>),
}

/// An implementation of [Provider] which uses a Starknet [JsonRpcClient] to query the World.
pub struct JsonRpcProvider<T> {
    /// Starknet JSON-RPC client.
    client: JsonRpcClient<T>,
    /// The address of the World contract.
    world_address: FieldElement,
    /// The block id to query the World with.
    block_id: BlockId,
}

impl<T> JsonRpcProvider<T>
where
    T: JsonRpcTransport + Sync + Send,
{
    pub fn new(client: JsonRpcClient<T>, world_address: FieldElement) -> Self {
        Self { client, world_address, block_id: BlockId::Tag(BlockTag::Latest) }
    }

    fn world(&self) -> WorldContractReader<'_, JsonRpcClient<T>> {
        WorldContractReader::new(self.world_address, &self.client)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T> Provider for JsonRpcProvider<T>
where
    T: JsonRpcTransport + Sync + Send,
{
    type Error = JsonRpcProviderError<JsonRpcClientError<T::Error>>;

    async fn component(&self, name: &str) -> Result<FieldElement, Self::Error> {
        let world = self.world();
        let class_hash = world
            .component(name, self.block_id)
            .await
            .map_err(JsonRpcProviderError::ComponetReader)?
            .class_hash();
        Ok(class_hash)
    }

    async fn entity(
        &self,
        component: &str,
        keys: Vec<FieldElement>,
    ) -> Result<Vec<FieldElement>, Self::Error> {
        let world = self.world();
        let component = world
            .component(component, self.block_id)
            .await
            .map_err(JsonRpcProviderError::ComponetReader)?;
        component.entity(keys, self.block_id).await.map_err(JsonRpcProviderError::ComponetReader)
    }
}
