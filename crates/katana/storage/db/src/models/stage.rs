use katana_primitives::block::BlockNumber;
use serde::{Deserialize, Serialize};

/// Unique identifier for a pipeline stage.
pub type StageId = String;

/// Pipeline stage checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[cfg_attr(test, derive(::arbitrary::Arbitrary))]
pub struct StageCheckpoint {
    /// The block number that the stage has processed up to.
    pub block: BlockNumber,
}
