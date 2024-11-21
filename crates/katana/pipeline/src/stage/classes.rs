use std::time::Duration;

use katana_primitives::block::BlockNumber;
use katana_primitives::class::{CasmContractClass, ClassHash, CompiledClass, ContractClass};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::state_update::StateUpdateProvider;
use starknet::providers::sequencer::models::{BlockId, DeployedClass};
use starknet::providers::{ProviderError, SequencerGatewayProvider};
use tracing::debug;

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
    ) -> Result<(ContractClass, Option<CompiledClass>), Error> {
        let block_id = BlockId::Number(block);

        let (class, casm) = tokio::join!(
            self.feeder_gateway.get_class_by_hash(hash, block_id),
            self.feeder_gateway.get_compiled_class_by_class_hash(hash, block_id)
        );

        let (class, casm) = (class?, casm?);

        let (sierra, casm) = match class {
            DeployedClass::LegacyClass(legacy) => {
                // let (.., legacy) = legacy_rpc_to_class(&legacy).unwrap();
                // (ContractClass::Legacy(legacy), None)

                todo!()
            }

            DeployedClass::SierraClass(sierra) => {
                // TODO: change this shyte
                let value = serde_json::to_value(casm).unwrap();
                let casm = serde_json::from_value::<CasmContractClass>(value).unwrap();
                let compiled = CompiledClass::Class(casm);

                (ContractClass::Class(sierra), Some(compiled))
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
                debug!(target: "pipeline", "Fetching class artifacts for class hash {class_hash:#x}");

                // 1. fetch sierra and casm class from fgw
                let (class, compiled) = self.get_class(*class_hash, i).await?;

                self.provider.set_class(*class_hash, class)?;
                if let Some(casm) = compiled {
                    self.provider.set_compiled_class(*class_hash, casm)?;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        Ok(())
    }
}
