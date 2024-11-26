use std::time::Duration;

use anyhow::Result;
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{
    CasmContractClass, ClassHash, CompiledClass, ContractClass, SierraContractClass,
};
use katana_primitives::conversion::rpc::{legacy_rpc_to_class, StarknetRsLegacyContractClass};
use katana_provider::traits::contract::{ContractClassWriter, ContractClassWriterExt};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_rpc_types::class::RpcSierraContractClass;
use starknet::providers::sequencer::models::{BlockId, DeployedClass};
use starknet::providers::{ProviderError, SequencerGatewayProvider};
use tracing::info;

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
    async fn get_class(&self, hash: ClassHash, block: BlockNumber) -> Result<ContractClass, Error> {
        let class = self.feeder_gateway.get_class_by_hash(hash, BlockId::Number(block)).await?;

        let class = match class {
            DeployedClass::LegacyClass(legacy) => {
                let class = to_inner_legacy_class(legacy).unwrap();
                class
            }

            // TODO: implement our own fgw client using our own types for easier conversion
            DeployedClass::SierraClass(sierra) => {
                let rpc_class = RpcSierraContractClass::try_from(sierra).unwrap();
                let class = SierraContractClass::try_from(rpc_class).unwrap();
                ContractClass::Class(class)
            }
        };

        Ok(class)
    }
}

#[async_trait::async_trait]
impl<P> Stage for Classes<P>
where
    P: StateUpdateProvider + ContractClassWriter + ContractClassWriterExt,
{
    fn id(&self) -> &'static str {
        "Classes"
    }

    async fn execute(&mut self, input: &StageExecutionInput) -> StageResult {
        for i in input.from..=input.to {
            // loop thru all the class hashes in the current block
            let class_hashes = self.provider.declared_classes(i.into())?.unwrap();

            // TODO: do this in parallel
            for hash in class_hashes.keys() {
                info!(target: "pipeline", class_hash = format!("{hash:#x}"), "Fetching class artifacts.");

                // 1. fetch sierra and casm class from fgw
                let class = self.get_class(*hash, i).await?;
                self.provider.set_class(*hash, class)?;

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }

        Ok(())
    }
}

fn to_inner_legacy_class(class: StarknetRsLegacyContractClass) -> Result<ContractClass> {
    let value = serde_json::to_value(class)?;
    let class = serde_json::from_value::<katana_primitives::class::LegacyContractClass>(value)?;
    Ok(ContractClass::Legacy(class))
}
