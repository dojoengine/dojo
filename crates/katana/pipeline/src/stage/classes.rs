use std::time::Duration;

use katana_primitives::block::BlockNumber;
use katana_primitives::class::{
    ClassHash, CompiledClass, DeprecatedCompiledClass, FlattenedSierraClass,
};
use katana_primitives::conversion::rpc::flattened_sierra_to_compiled_class;
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::state_update::StateUpdateProvider;
use starknet::providers::sequencer::models::{BlockId, DeployedClass};
use starknet::providers::{ProviderError, SequencerGatewayProvider};

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
    async fn get_class(
        &self,
        hash: ClassHash,
        block: BlockNumber,
    ) -> Result<(Option<FlattenedSierraClass>, CompiledClass), Error> {
        let block_id = BlockId::Number(block);

        let class = self.feeder_gateway.get_class_by_hash(hash, block_id).await?;

        let (sierra, casm) = match class {
            DeployedClass::LegacyClass(legacy) => {
                // TODO: change this shit
                let class = serde_json::to_value(legacy).unwrap();
                let class = serde_json::from_value::<DeprecatedCompiledClass>(class).unwrap();
                (None, CompiledClass::Deprecated(class))
            }
            DeployedClass::SierraClass(sierra) => {
                let (_, _, class) = flattened_sierra_to_compiled_class(&sierra).unwrap();
                (Some(sierra), class)
            }
        };

        Ok((sierra, casm))
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
                let (sierra, compiled) = self.get_class(*class_hash, i).await?;

                // 2. store the classes
                if let Some(sierra) = sierra {
                    self.provider.set_sierra_class(*class_hash, sierra)?;
                }

                self.provider.set_class(*class_hash, compiled)?;

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        Ok(())
    }
}
