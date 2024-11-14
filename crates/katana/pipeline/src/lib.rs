#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod stage;

use core::future::IntoFuture;

use futures::future::BoxFuture;
use katana_primitives::block::BlockNumber;
use stage::Stage;
use tracing::{error, info};

/// The result of a pipeline execution.
pub type PipelineResult = Result<(), Error>;

/// The future type for [Pipeline]'s implementation of [IntoFuture].
pub type PipelineFut = BoxFuture<'static, PipelineResult>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Stage(#[from] stage::Error),
}

/// Manages the execution of stages.
///
/// The pipeline drives the execution of stages, running each stage to completion in the order they
/// were added.
///
/// Inspired by [`reth`]'s staged sync pipeline.
///
/// [`reth`]: https://github.com/paradigmxyz/reth/blob/c7aebff0b6bc19cd0b73e295497d3c5150d40ed8/crates/stages/api/src/pipeline/mod.rs#L66
pub struct Pipeline {
    stages: Vec<Box<dyn Stage>>,
}

impl Pipeline {
    /// Create a new empty pipeline.
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Insert a new stage into the pipeline.
    pub fn add_stage(&mut self, stage: Box<dyn Stage>) {
        self.stages.push(stage);
    }

    /// Start the pipeline.
    pub async fn run(&mut self) -> PipelineResult {
        for stage in &mut self.stages {
            info!(target: "pipeline", id = %stage.id(), "Executing stage.");
            stage.execute().await?;
        }
        info!(target: "pipeline", "Pipeline finished.");
        Ok(())
    }
}

impl IntoFuture for Pipeline {
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

impl core::default::Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pipeline")
            .field("stages", &self.stages.iter().map(|s| s.id()).collect::<Vec<_>>())
            .finish()
    }
}
