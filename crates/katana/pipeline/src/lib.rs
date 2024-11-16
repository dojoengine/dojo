#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod stage;

use core::future::IntoFuture;

use futures::future::BoxFuture;
use katana_primitives::block::BlockNumber;
use katana_provider::error::ProviderError;
use katana_provider::traits::stage::StageCheckpointProvider;
use stage::{Stage, StageExecutionInput, StageExecutionOutput};
use tracing::{error, info};

/// The result of a pipeline execution.
pub type PipelineResult = Result<(), Error>;

/// The future type for [Pipeline]'s implementation of [IntoFuture].
pub type PipelineFut = BoxFuture<'static, PipelineResult>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Stage not found: {id}")]
    StageNotFound { id: String },

    #[error(transparent)]
    Stage(#[from] stage::Error),

    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// Manages the execution of stages.
///
/// The pipeline drives the execution of stages, running each stage to completion in the order they
/// were added.
///
/// Inspired by [`reth`]'s staged sync pipeline.
///
/// [`reth`]: https://github.com/paradigmxyz/reth/blob/c7aebff0b6bc19cd0b73e295497d3c5150d40ed8/crates/stages/api/src/pipeline/mod.rs#L66
pub struct Pipeline<P> {
    tip: BlockNumber,
    stages: Vec<Box<dyn Stage>>,
    provider: P,
}

impl<P> Pipeline<P> {
    /// Create a new empty pipeline.
    pub fn new(provider: P, tip: BlockNumber) -> Self {
        Self { stages: Vec::new(), tip, provider }
    }

    /// Insert a new stage into the pipeline.
    pub fn add_stage(&mut self, stage: Box<dyn Stage>) {
        self.stages.push(stage);
    }
}

impl<P: StageCheckpointProvider> Pipeline<P> {
    /// Start the pipeline.
    pub async fn run(&mut self) -> PipelineResult {
        let mut input = StageExecutionInput { from: 0, to: self.tip };

        for stage in &mut self.stages {
            let id = stage.id();
            let checkpoint = self.provider.checkpoint(id)?.unwrap_or_default();

            if checkpoint > input.to {
                info!(target: "pipeline", %id, "Skipping stage.");
                continue;
            } else {
                input.from = checkpoint;
            }

            info!(target: "pipeline", %id, "Executing stage.");
            let StageExecutionOutput { last_block_processed } = stage.execute(&input).await?;

            // TODO: store the stage checkpoint in the db based on
            // the latest block number the stage has processed
            self.provider.set_checkpoint(id, last_block_processed)?;

            input.to = last_block_processed;
        }

        info!(target: "pipeline", "Pipeline finished.");

        Ok(())
    }
}

impl<P> IntoFuture for Pipeline<P>
where
    P: StageCheckpointProvider + 'static,
{
    type Output = PipelineResult;
    type IntoFuture = PipelineFut;

    fn into_future(mut self) -> Self::IntoFuture {
        Box::pin(async move {
            self.run().await.inspect_err(|error| {
                error!(target: "pipeline", %error, "Pipeline failed.");
            })
        })
    }
}

// impl core::default::Default for Pipeline {
//     fn default() -> Self {
//         Self::new()
//     }
// }

impl<P> core::fmt::Debug for Pipeline<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pipeline")
            .field("tip", &self.tip)
            .field("stages", &self.stages.iter().map(|s| s.id()).collect::<Vec<_>>())
            .finish()
    }
}
