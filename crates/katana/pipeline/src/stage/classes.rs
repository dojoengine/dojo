use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use backon::{ExponentialBuilder, Retryable};
use katana_feeder_gateway::client::{self, SequencerGateway};
use katana_primitives::block::{BlockIdOrTag, BlockNumber};
use katana_primitives::class::{ClassHash, ContractClass};
use katana_provider::error::ProviderError;
use katana_provider::traits::contract::{ContractClassWriter, ContractClassWriterExt};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_rpc_types::class::ConversionError;
use tracing::{debug, error, warn};

use super::{Stage, StageExecutionInput, StageResult};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing declared classes for block {block}")]
    MissingBlockDeclaredClasses {
        /// The block number whose declared classes are missing.
        block: BlockNumber,
    },

    /// Error returnd by the client used to download the classes from.
    #[error(transparent)]
    Gateway(#[from] client::Error),

    /// Error that can occur when converting the classes types to the internal types.
    #[error(transparent)]
    Conversion(#[from] ConversionError),

    #[error(transparent)]
    Provider(#[from] ProviderError),
}

#[derive(Debug)]
pub struct Classes<P> {
    provider: P,
    downloader: Downloader,
}

impl<P> Classes<P> {
    pub fn new(provider: P, feeder_gateway: SequencerGateway, download_batch_size: usize) -> Self {
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
        let mut classes: Vec<(ClassHash, ContractClass)> = Vec::new();

        for block in input.from..=input.to {
            // get the classes declared at block `i`
            let class_hashes = self
                .provider
                .declared_classes(block.into())?
                .ok_or(Error::MissingBlockDeclaredClasses { block })?;
            let class_hashes = class_hashes.keys().copied().collect::<Vec<_>>();

            // fetch the classes artifacts
            let class_artifacts = self.downloader.download_classes(&class_hashes, block).await?;
            classes.extend(class_hashes.into_iter().zip(class_artifacts));
        }

        if !classes.is_empty() {
            debug!(target: "stage", id = self.id(), total = %classes.len(), "Storing class artifacts.");
            for (hash, class) in classes {
                self.provider.set_class(hash, class)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Downloader {
    batch_size: usize,
    client: Arc<SequencerGateway>,
}

impl Downloader {
    fn new(client: SequencerGateway, batch_size: usize) -> Self {
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
            .when(|error| matches!(error, Error::Gateway(client::Error::RateLimited)))
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
        let class = self.client.get_class(hash, BlockIdOrTag::Number(block)).await.inspect_err(
            |error| {
                if !error.is_rate_limited() {
	                error!(target: "pipeline", %error, %block, class = %format!("{hash:#x}"), "Fetching class.")
                }
            },
        )?;
        Ok(class.try_into()?)
    }
}
