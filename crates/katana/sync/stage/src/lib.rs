#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use katana_primitives::block::BlockNumber;
use katana_provider::error::ProviderError;

mod blocks;
mod classes;
mod sequencing;

pub use blocks::Blocks;
pub use classes::Classes;
pub use sequencing::Sequencing;

/// The result type of a stage execution. See [Stage::execute].
pub type StageResult = Result<(), Error>;

#[derive(Debug, Default, Clone)]
pub struct StageExecutionInput {
    pub from: BlockNumber,
    pub to: BlockNumber,
}

#[derive(Debug, Default)]
pub struct StageExecutionOutput {
    pub last_block_processed: BlockNumber,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Provider(#[from] ProviderError),

    /// Errors that could happen during the execution of the [`Blocks`](blocks::Blocks) stage.
    #[error(transparent)]
    Blocks(#[from] blocks::Error),

    /// Errors that could happen during the execution of the [`Classes`](classes::Classes) stage.
    #[error(transparent)]
    Classes(#[from] classes::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait Stage: Send + Sync {
    /// Returns the id which uniquely identifies the stage.
    fn id(&self) -> &'static str;

    /// Executes the stage.
    async fn execute(&mut self, input: &StageExecutionInput) -> StageResult;
}
