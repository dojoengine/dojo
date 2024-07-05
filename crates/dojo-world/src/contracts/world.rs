use std::result::Result;

pub use abigen::world::{
    ContractDeployed, ContractUpgraded, Event as WorldEvent, ModelRegistered, WorldContract,
    WorldContractReader,
};
use starknet::providers::Provider;

use super::model::{ModelError, ModelRPCReader};
use super::naming;

#[cfg(test)]
#[path = "world_test.rs"]
pub(crate) mod test;

pub mod abigen {
    pub mod world {
        pub use crate::contracts::abi::world::*;
    }
}

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
}
