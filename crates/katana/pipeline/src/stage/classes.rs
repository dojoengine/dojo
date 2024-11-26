use std::sync::Arc;
use std::time::Duration;

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
use tracing::{debug, warn};

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
    pub fn new(
        provider: P,
        feeder_gateway: SequencerGatewayProvider,
        download_batch_size: usize,
    ) -> Self {
        let downloader = Downloader::new(feeder_gateway, download_batch_size);
        Self { provider, downloader }
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
            // get the classes declared at block `i`
            let class_hashes = self.provider.declared_classes(i.into())?.unwrap();
            let class_hashes = class_hashes.keys().map(|hash| *hash).collect::<Vec<_>>();

            // fetch the classes artifacts
            let classes = self.downloader.download_classes(&class_hashes, i).await?;

            if !classes.is_empty() {
                debug!(target: "stage", id = %self.id(), total = %classes.len(), "Storing classes to storage.");
                for (hash, class) in class_hashes.iter().zip(classes) {
                    self.provider.set_class(*hash, class)?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Downloader {
    batch_size: usize,
    client: Arc<SequencerGatewayProvider>,
}

impl Downloader {
    fn new(client: SequencerGatewayProvider, batch_size: usize) -> Self {
        Self { client: Arc::new(client), batch_size }
    }

    async fn download_classes(
        &self,
        hashes: &[ClassHash],
        block: BlockNumber,
    ) -> Result<Vec<ContractClass>, Error> {
        debug!(total = %hashes.len(), %block, "Downloading classes.");
        let mut classes = Vec::with_capacity(hashes.len());

        for chunk in hashes.chunks(self.batch_size) {
            let batch = self.fetch_classes_with_retry(chunk, block).await?;
            classes.extend(batch);
        }

        Ok(classes)
    }

    async fn fetch_classes_with_retry(
        &self,
        classes: &[ClassHash],
        block: BlockNumber,
    ) -> Result<Vec<ContractClass>, Error> {
        let request = || async move { self.clone().fetch_classes(classes, block).await };

        // Retry only when being rate limited
        let backoff = ExponentialBuilder::default().with_min_delay(Duration::from_secs(3));
        let result = request
            .retry(backoff)
            .notify(|error, _| {
                warn!(target: "pipeline", %error, "Retrying class download.");
            })
            .await?;

        Ok(result)
    }

    async fn fetch_classes(
        &self,
        classes: &[ClassHash],
        block: BlockNumber,
    ) -> Result<Vec<ContractClass>, Error> {
        let mut requests = Vec::with_capacity(classes.len());

        for class in classes {
            requests.push(self.fetch_class(*class, block));
        }

        let results = futures::future::join_all(requests).await;
        results.into_iter().collect()
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
