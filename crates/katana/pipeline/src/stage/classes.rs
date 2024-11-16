use std::time::Duration;

use katana_primitives::block::BlockNumber;
use katana_primitives::class::ClassHash;
use katana_provider::traits::{contract::ContractClassWriter, state_update::StateUpdateProvider};
use starknet::providers::{sequencer::models::BlockId, ProviderError, SequencerGatewayProvider};

use super::{Stage, StageExecutionInput, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gateway(#[from] ProviderError),
}

#[derive(Debug)]
pub struct Classes<P> {
    provider: P,
    feeder_gateway: SequencerGatewayProvider,
}

impl<P> Classes<P> {
    pub fn new(provider: P, feeder_gateway: SequencerGatewayProvider) -> Self {
        Self { provider, feeder_gateway }
    }
}

impl<P> Classes<P>
where
    P: StateUpdateProvider + ContractClassWriter,
{
    #[allow(deprecated)]
    async fn get_class(&self, hash: ClassHash, block: BlockNumber) -> Result<(), Error> {
        let block_id = BlockId::Number(block);

        let _ = self.feeder_gateway.get_class_by_hash(hash, block_id).await?;
        let _ = self.feeder_gateway.get_compiled_class_by_class_hash(hash, block_id).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl<P> Stage for Classes<P>
where
    P: StateUpdateProvider + ContractClassWriter,
{
    fn id(&self) -> &'static str {
        "Classes"
    }

    async fn execute(&mut self, input: &StageExecutionInput) -> StageResult {
        for i in input.from..=input.to {
            // loop thru all the class hashes in the current block
            let class_hashes = self.provider.declared_classes(i.into())?.unwrap();

            // TODO: do this in parallel
            for class_hash in class_hashes.keys() {
                // 1. fetch sierra and casm class from fgw
                let _ = self.get_class(*class_hash, i).await?;

                // 2. store the classes
                // self.provider.set_compiled_class(class_hash, class)?;
                // self.provider.set_sierra_class(class_hash, class)?;

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        Ok(())
    }
}
