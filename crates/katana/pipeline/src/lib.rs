#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod stage;

use core::future::IntoFuture;
use std::borrow::Borrow;
use std::sync::Arc;

use futures::future::BoxFuture;
use katana_primitives::block::BlockNumber;
use katana_provider::error::ProviderError;
use katana_provider::traits::stage::StageCheckpointProvider;
use parking_lot::{Condvar, Mutex};
use stage::{Stage, StageExecutionInput};
use tokio::sync::watch;
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

#[derive(Debug)]
pub struct PipelineHandle {
    status: Arc<(Mutex<PipelineStatus>, Condvar)>,
    tx: watch::Sender<Option<BlockNumber>>,
}

impl PipelineHandle {
    pub fn set_tip(&self, tip: BlockNumber) {
        self.tx.send(Some(tip)).expect("channel closed");
    }

    pub fn stopped(&self) {
        while *self.status.0.lock() != PipelineStatus::Stopped {
            self.status.1.wait(&mut self.status.0.lock());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStatus {
    NotStarted,
    Syncing,
    Stopped,
    Finished,
}

/// Syncing pipeline.
///
/// The pipeline drives the execution of stages, running each stage to completion in the order they
/// were added.
///
/// Inspired by [`reth`]'s staged sync pipeline.
///
/// [`reth`]: https://github.com/paradigmxyz/reth/blob/c7aebff0b6bc19cd0b73e295497d3c5150d40ed8/crates/stages/api/src/pipeline/mod.rs#L66
pub struct Pipeline<P> {
    status: Arc<(Mutex<PipelineStatus>, Condvar)>,
    chunk_size: u64,
    tip: watch::Receiver<Option<BlockNumber>>,
    provider: P,
    stages: Vec<Box<dyn Stage>>,
}

impl<P> Pipeline<P> {
    /// Create a new empty pipeline.
    pub fn new(provider: P, chunk_size: u64) -> (Self, PipelineHandle) {
        let (tx, rx) = watch::channel(None);
        let status = Arc::new((Mutex::new(PipelineStatus::NotStarted), Condvar::new()));

        let handle = PipelineHandle { tx, status: status.clone() };
        let pipeline = Self { stages: Vec::new(), tip: rx, provider, chunk_size, status };

        (pipeline, handle)
    }

    /// Insert a new stage into the pipeline.
    pub fn add_stage<S: Stage + 'static>(&mut self, stage: S) {
        self.stages.push(Box::new(stage));
    }

    /// Insert multiple stages into the pipeline.
    ///
    /// The stages will be executed in the order they are appear in the iterator.
    pub fn add_stages(&mut self, stages: impl Iterator<Item = Box<dyn Stage>>) {
        self.stages.extend(stages);
    }
}

impl<P: StageCheckpointProvider> Pipeline<P> {
    /// Run the pipeline in a loop.
    pub async fn run(&mut self) -> PipelineResult {
        let mut current_chunk_tip = self.chunk_size;

        loop {
            let tip = *self.tip.borrow_and_update();

            if let Some(tip) = tip {
                {
                    let mut status = self.status.0.lock();
                    if *status != PipelineStatus::Syncing {
                        *status = PipelineStatus::Syncing;
                    }
                }

                let until_block = current_chunk_tip.min(tip);
                self.run_once_until(until_block).await?;

                if current_chunk_tip < tip {
                    current_chunk_tip = (current_chunk_tip + self.chunk_size).min(tip);
                    continue;
                }
            }

            *self.status.0.lock() = PipelineStatus::Stopped;
            self.status.1.notify_one();

            println!("stopped");

            // wait until the tip has changed
            if self.tip.changed().await.is_err() {
                break;
            }
        }

        *self.status.0.lock() = PipelineStatus::Finished;
        info!(target: "pipeline", "Pipeline finished.");

        Ok(())
    }

    /// Run the pipeline once, until the given block number.
    async fn run_once_until(&mut self, to: BlockNumber) -> PipelineResult {
        for stage in &mut self.stages {
            let id = stage.id();

            // Get the checkpoint for the stage, otherwise default to block number 0
            let checkpoint = self.provider.checkpoint(id)?.unwrap_or_default();

            // Skip the stage if the checkpoint is greater than or equal to the target block number
            if checkpoint >= to {
                info!(target: "pipeline", %id, "Skipping stage.");
                continue;
            }

            info!(target: "pipeline", %id, from_block = %checkpoint, to_block = %to, "Executing stage.");

            // plus 1 because the checkpoint is inclusive
            let input = StageExecutionInput { from: checkpoint + 1, to };
            stage.execute(&input).await?;
            self.provider.set_checkpoint(id, to)?;
        }

        Ok(())
    }

    #[cfg(test)]
    pub async fn run_until(&mut self, to: BlockNumber) -> PipelineResult {
        let mut current_chunk_tip = self.chunk_size.min(to);

        loop {
            self.run_once_until(current_chunk_tip).await?;

            if current_chunk_tip >= to {
                break;
            } else {
                current_chunk_tip = (current_chunk_tip + self.chunk_size).min(to);
            }
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

impl<P> core::fmt::Debug for Pipeline<P>
where
    P: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pipeline")
            .field("tip", &self.tip)
            .field("provider", &self.provider)
            .field("chunk_size", &self.chunk_size)
            .field("stages", &self.stages.iter().map(|s| s.id()).collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use katana_provider::test_utils::test_provider;
    use katana_provider::traits::stage::StageCheckpointProvider;

    use super::{Pipeline, Stage, StageExecutionInput};
    use crate::stage::StageResult;

    struct MockStage;

    #[async_trait::async_trait]
    impl Stage for MockStage {
        fn id(&self) -> &'static str {
            "Mock"
        }

        async fn execute(&mut self, _: &StageExecutionInput) -> StageResult {
            Ok(())
        }
    }

    #[tokio::test]
    async fn stage_checkpoint() {
        let provider = test_provider();

        let (mut pipeline, handle) = Pipeline::new(&provider, 10);
        pipeline.add_stage(MockStage);

        // check that the checkpoint was set
        let initial_checkpoint = provider.checkpoint("Mock").unwrap();
        assert_eq!(initial_checkpoint, None);

        pipeline.run_once_until(5).await.expect("failed to run the pipeline once");

        // check that the checkpoint was set
        let actual_checkpoint = provider.checkpoint("Mock").unwrap();
        assert_eq!(actual_checkpoint, Some(5));

        pipeline.run_once_until(10).await.expect("failed to run the pipeline once");

        // check that the checkpoint was set
        let actual_checkpoint = provider.checkpoint("Mock").unwrap();
        assert_eq!(actual_checkpoint, Some(10));

        pipeline.run_once_until(10).await.expect("failed to run the pipeline once");

        // check that the checkpoint doesn't change
        let actual_checkpoint = provider.checkpoint("Mock").unwrap();
        assert_eq!(actual_checkpoint, Some(10));
    }
}
