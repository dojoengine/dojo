mod blocks;
mod sequencing;

use katana_primitives::block::BlockNumber;
pub use sequencing::Sequencing;

/// The result type of a stage execution. See [Stage::execute].
pub type StageResult = Result<(), Error>;

#[derive(Debug, Clone, Copy)]
pub enum StageId {
    Blocks,
    Sequencing,
}

impl core::fmt::Display for StageId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StageId::Sequencing => write!(f, "Sequencing"),
            StageId::Blocks => write!(f, "Blocks"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Blocks(#[from] blocks::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait Stage: Send + Sync {
    /// Returns the id which uniquely identifies the stage.
    fn id(&self) -> StageId;

    /// Executes the stage.
    async fn execute(&mut self) -> StageResult;
}
