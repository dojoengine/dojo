use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::state::errors::StateError;
use blockifier::transaction::errors::TransactionExecutionError;
use starknet::providers::jsonrpc::models::BlockId;
use starknet_api::transaction::TransactionHash;
use starknet_api::StarknetApiError;

#[derive(Debug, thiserror::Error)]
pub enum SequencerError {
    #[error("Block {0:?} not found.")]
    BlockNotFound(BlockId),
    #[error("State update for block {0:?} not found.")]
    StateUpdateNotFound(BlockId),
    #[error("State for block {0:?} not found.")]
    StateNotFound(BlockId),
    #[error("Transaction with {0} hash not found.")]
    TxnNotFound(TransactionHash),
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
}
