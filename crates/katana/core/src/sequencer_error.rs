use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::transaction::errors::TransactionExecutionError;
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::contract::ContractAddress;
use katana_primitives::event::ContinuationTokenError;
use katana_provider::error::ProviderError;

#[derive(Debug, thiserror::Error)]
pub enum SequencerError {
    #[error("Block {0:?} not found.")]
    BlockNotFound(BlockIdOrTag),
    #[error("Contract address {0} not found.")]
    ContractNotFound(ContractAddress),
    #[error("State for block {0:?} not found.")]
    StateNotFound(BlockIdOrTag),
    #[error(transparent)]
    TransactionExecution(#[from] TransactionExecutionError),
    #[error(transparent)]
    EntryPointExecution(#[from] EntryPointExecutionError),
    #[error("Wait for pending transactions.")]
    PendingTransactions,
    #[error(transparent)]
    ContinuationToken(#[from] ContinuationTokenError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
}
