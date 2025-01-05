use std::result::Result;

use starknet::core::types::BlockId;
use starknet::providers::Provider;

pub use super::abigen::world::{
    ContractRegistered, ContractUpgraded, Event as WorldEvent, ModelRegistered, WorldContract,
    WorldContractReader,
};
use super::model::{ModelError, ModelRPCReader};
use super::naming;

// #[cfg(test)]
// #[path = "world_test.rs"]
// pub(crate) mod test;

impl<P> WorldContractReader<P>
where
    P: Provider + Sync + Send,
{
    pub async fn model_reader_with_tag(
        &self,
        tag: &str,
    ) -> Result<ModelRPCReader<'_, P>, ModelError> {
        let (namespace, name) =
            naming::split_tag(tag).map_err(|e| ModelError::TagError(e.to_string()))?;
        ModelRPCReader::new(&namespace, &name, self).await
    }

    pub async fn model_reader(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<ModelRPCReader<'_, P>, ModelError> {
        ModelRPCReader::new(namespace, name, self).await
    }

    pub async fn model_reader_with_block(
        &self,
        namespace: &str,
        name: &str,
        block_id: BlockId,
    ) -> Result<ModelRPCReader<'_, P>, ModelError> {
        ModelRPCReader::new_with_block(namespace, name, self, block_id).await
    }
}
