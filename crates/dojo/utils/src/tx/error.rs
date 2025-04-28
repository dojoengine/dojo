use starknet::accounts::AccountError;
use starknet::core::types::contract::ComputeClassHashError;
use starknet::core::types::{StarknetError, TransactionExecutionErrorData};
use starknet::providers::ProviderError;
use thiserror::Error;

use crate::TransactionWaitingError;

#[derive(Debug, Error)]
pub enum TransactionError<S>
where
    S: std::error::Error,
{
    #[error(transparent)]
    SigningError(S),
    #[error(transparent)]
    Provider(ProviderError),
    #[error("Transaction execution error")]
    TransactionExecution(TransactionExecutionErrorData),
    #[error("{0}")]
    TransactionValidation(String),
    #[error(transparent)]
    TransactionWaiting(#[from] TransactionWaitingError),
    #[error(transparent)]
    ComputeClassHash(#[from] ComputeClassHashError),
    #[error("Fee calculation overflow")]
    FeeOutOfRange,
}

impl<S> From<AccountError<S>> for TransactionError<S>
where
    S: std::error::Error,
{
    fn from(value: AccountError<S>) -> Self {
        match value {
            AccountError::Signing(e) => TransactionError::SigningError(e),
            AccountError::Provider(e) => Self::from(e),
            AccountError::ClassHashCalculation(e) => TransactionError::ComputeClassHash(e),
            AccountError::FeeOutOfRange => TransactionError::FeeOutOfRange,
        }
    }
}

impl<S> From<ProviderError> for TransactionError<S>
where
    S: std::error::Error,
{
    fn from(value: ProviderError) -> Self {
        match value {
            ProviderError::StarknetError(StarknetError::TransactionExecutionError(te)) => {
                TransactionError::TransactionExecution(te)
            }
            ProviderError::StarknetError(StarknetError::ValidationFailure(ve)) => {
                TransactionError::TransactionValidation(ve)
            }
            _ => TransactionError::Provider(value),
        }
    }
}
