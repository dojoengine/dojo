use std::sync::Arc;

use anyhow::Result;
use backon::{ExponentialBuilder, Retryable};
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, ContractClass, SierraContractClass};
use katana_primitives::conversion::rpc::StarknetRsLegacyContractClass;
use katana_provider::traits::contract::{ContractClassWriter, ContractClassWriterExt};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_rpc_types::class::RpcSierraContractClass;
use starknet::providers::sequencer::models::{BlockId, DeployedClass};
use starknet::providers::{ProviderError, SequencerGatewayProvider};
use tracing::warn;

use super::{Stage, StageExecutionInput, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gateway(#[from] ProviderError),
}

#[derive(Debug)]
pub struct Classes<P> {
    provider: P,
    downloader: Downloader,
}

impl<P> Classes<P> {
    pub fn new(provider: P, feeder_gateway: SequencerGatewayProvider) -> Self {
        Self { provider, downloader: Downloader::new(feeder_gateway) }
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
            let class_hashes = self.provider.declared_classes(i.into())?.unwrap();
            let class_hashes = class_hashes.keys().map(|hash| *hash).collect::<Vec<_>>();

            let classes = self.downloader.fetch_classes(&class_hashes, i).await?;
            for (hash, class) in classes {
                self.provider.set_class(hash, class)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Downloader {
    client: Arc<SequencerGatewayProvider>,
}

impl Downloader {
    fn new(client: SequencerGatewayProvider) -> Self {
        Self { client: Arc::new(client) }
    }

    async fn fetch_classes(
        &self,
        classes: &[ClassHash],
        block: BlockNumber,
    ) -> Result<Vec<(ClassHash, ContractClass)>, Error> {
        let mut all_results = Vec::with_capacity(classes.len());

        for hash in classes {
            let mut futures = Vec::new();

            futures.push(self.fetch_class_with_retry(*hash, block));
            let batch_results = futures::future::join_all(futures).await;

            all_results.extend(batch_results);
        }

        all_results.into_iter().collect()
    }

    async fn fetch_class_with_retry(
        &self,
        hash: ClassHash,
        block: BlockNumber,
    ) -> Result<(ClassHash, ContractClass), Error> {
        let request = || async move {
            #[allow(deprecated)]
            self.clone().fetch_class(hash, block).await
        };

        // Retry only when being rate limited
        let result = request
            .retry(ExponentialBuilder::default())
            .when(|e| matches!(e, Error::Gateway(ProviderError::RateLimited)))
            .notify(|error, _| {
                warn!(target: "pipeline", hash = format!("{hash:#x}"), %block, %error, "Retrying class download.");
            })
            .await?;

        Ok((hash, result))
    }

    async fn fetch_class(
        &self,
        hash: ClassHash,
        block: BlockNumber,
    ) -> Result<ContractClass, Error> {
        #[allow(deprecated)]
        let class = self.client.get_class_by_hash(hash, BlockId::Number(block)).await?;

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

fn to_inner_legacy_class(class: StarknetRsLegacyContractClass) -> Result<ContractClass> {
    let value = serde_json::to_value(class)?;
    let class = serde_json::from_value::<katana_primitives::class::LegacyContractClass>(value)?;
    Ok(ContractClass::Legacy(class))
}
