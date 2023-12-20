use std::result::Result;

pub use abigen::world::{WorldContract, WorldContractReader};
use cainome::cairo_serde::Result as CainomeResult;
use starknet::core::types::FieldElement;
use starknet::providers::Provider;

use super::model::{ModelError, ModelRPCReader};

#[cfg(test)]
#[path = "world_test.rs"]
pub(crate) mod test;

#[cfg(not(doctest))]
pub mod abigen {
    pub mod world {
        use cainome::rs::abigen;
        abigen!(WorldContract, "crates/dojo-world/src/contracts/abi/world.json");
    }

    pub mod executor {
        use cainome::rs::abigen;
        abigen!(ExecutorContract, "crates/dojo-world/src/contracts/abi/executor.json");
    }
}

#[cfg(doctest)]
pub mod abigen {
    pub mod world {
        use cainome::rs::abigen;
        abigen!(WorldContract, "src/contracts/abi/world.json");
    }

    pub mod executor {
        use cainome::rs::abigen;
        abigen!(ExecutorContract, "src/contracts/abi/executor.json");
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

impl<P> WorldContractReader<P>
where
    P: Provider + Sync + Send,
{
    pub async fn executor_call(
        &self,
        class_hash: FieldElement,
        entry_point: FieldElement,
        calldata: Vec<FieldElement>,
    ) -> CainomeResult<Vec<FieldElement>> {
        let executor_address = self.executor().block_id(self.block_id).call().await?;

        let executor =
            abigen::executor::ExecutorContractReader::new(executor_address.into(), &self.provider);

        let res = executor
            .call(&class_hash.into(), &entry_point, &calldata)
            .block_id(self.block_id)
            .call()
            .await?;

        Ok(res)
    }
}
