use katana_primitives::block::BlockNumber;
use katana_provider::error::ProviderError;

mod sequencing;

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
