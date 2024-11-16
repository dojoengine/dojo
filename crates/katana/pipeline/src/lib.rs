#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod stage;

use core::future::IntoFuture;

use futures::future::BoxFuture;
use katana_primitives::block::BlockNumber;
use katana_provider::error::ProviderError;
use katana_provider::traits::stage::StageCheckpointProvider;
use stage::{Stage, StageExecutionInput};
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

    pub fn add_stages(&mut self, stages: impl Iterator<Item = Box<dyn Stage>>) {
        self.stages.extend(stages);
    }
}

impl<P> Pipeline<P>
where
    P: StageCheckpointProvider,
{
    /// Start the pipeline.
    pub async fn run(&mut self) -> PipelineResult {
        for stage in &mut self.stages {
            let id = stage.id();
            let checkpoint = self.provider.checkpoint(id)?.unwrap_or_default();

            if checkpoint > self.tip {
                info!(target: "pipeline", %id, "Skipping stage.");
                continue;
            }

            info!(target: "pipeline", %id, "Executing stage.");

            let input = StageExecutionInput { from: checkpoint, to: self.tip };
            let output = stage.execute(&input).await?;

            self.provider.set_checkpoint(id, output.last_block_processed)?;
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

impl<P> core::fmt::Debug for Pipeline<P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pipeline")
            .field("tip", &self.tip)
            .field("stages", &self.stages.iter().map(|s| s.id()).collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use katana_provider::test_utils::test_provider;
    use katana_provider::traits::stage::StageCheckpointProvider;

    use super::{Pipeline, Stage, StageExecutionInput};
    use crate::stage::{StageExecutionOutput, StageResult};

    struct MockStage;

    #[async_trait::async_trait]
    impl Stage for MockStage {
        fn id(&self) -> &'static str {
            "Mock"
        }

        async fn execute(&mut self, _: &StageExecutionInput) -> StageResult {
            Ok(StageExecutionOutput { last_block_processed: 10 })
        }
    }

    #[tokio::test]
    async fn stage_checkpoint() {
        let provider = test_provider();

        let mut pipeline = Pipeline::new(&provider, 10);
        pipeline.add_stage(Box::new(MockStage));

        // check that the checkpoint was set
        let initial_checkpoint = provider.checkpoint("Mock").unwrap();
        assert_eq!(initial_checkpoint, None);

        pipeline.run().await.expect("pipeline failed");

        // check that the checkpoint was set
        let actual_checkpoint = provider.checkpoint("Mock").unwrap();
        assert_eq!(actual_checkpoint, Some(10));
    }
}
