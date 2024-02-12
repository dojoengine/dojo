use std::result::Result;

pub use abigen::world::{
    ContractDeployed, ContractUpgraded, Event as WorldEvent, ModelRegistered, WorldContract,
    WorldContractReader,
};
use starknet::providers::Provider;

use super::model::{ModelError, ModelRPCReader};

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
    pub async fn model_reader(&self, name: &str) -> Result<ModelRPCReader<'_, P>, ModelError> {
        ModelRPCReader::new(name, self).await
    }
}
