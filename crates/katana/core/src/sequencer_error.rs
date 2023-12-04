use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::state::errors::StateError;
use blockifier::transaction::errors::TransactionExecutionError;
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::contract::ContractAddress;
use katana_primitives::transaction::TxHash;
use starknet_api::StarknetApiError;

use crate::utils::event::ContinuationTokenError;

#[derive(Debug, thiserror::Error)]
pub enum SequencerError {
    #[error("Block {0:?} not found.")]
    BlockNotFound(BlockIdOrTag),
    #[error("Contract address {0:?} not found.")]
    ContractNotFound(ContractAddress),
    #[error("State update for block {0:?} not found.")]
    StateUpdateNotFound(BlockIdOrTag),
    #[error("State for block {0:?} not found.")]
    StateNotFound(BlockIdOrTag),
    #[error("Transaction with {0} hash not found.")]
    TxnNotFound(TxHash),
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    TransactionExecution(#[from] TransactionExecutionError),
    #[error("Error converting {from} into {to}: {message}")]
    ConversionError { from: String, to: String, message: String },
    #[error(transparent)]
    StarknetApi(#[from] StarknetApiError),
    #[error(transparent)]
    EntryPointExecution(#[from] EntryPointExecutionError),
    #[error("Wait for pending transactions.")]
    PendingTransactions,
    #[error("Unsupported Transaction")]
    UnsupportedTransaction,
    #[error(transparent)]
    ContinuationToken(#[from] ContinuationTokenError),
    #[error("Error serializing state.")]
    StateSerialization,
    #[error("Required data unavailable")]
    DataUnavailable,
    #[error("Failed to decode state")]
    FailedToDecodeStateDump,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
