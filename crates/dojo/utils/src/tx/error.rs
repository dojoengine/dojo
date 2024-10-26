use starknet::accounts::AccountError;
use starknet::core::types::contract::{CompressProgramError, ComputeClassHashError};
use starknet::core::types::StarknetError;
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
    #[error("{0}")]
    TransactionExecution(String),
    #[error(transparent)]
    TransactionWaiting(#[from] TransactionWaitingError),
    #[error(transparent)]
    ComputeClassHash(#[from] ComputeClassHashError),
    #[error(transparent)]
    ClassCompression(#[from] CompressProgramError),
    #[error("Fee calculation overflow")]
    FeeOutOfRange,
}

impl<S> From<AccountError<S>> for TransactionError<S>
where
    S: std::error::Error,
{
    fn from(value: AccountError<S>) -> Self {
        println!("FROM A: {:?}", value);
        match value {
            AccountError::Signing(e) => TransactionError::SigningError(e),
            AccountError::Provider(e) => Self::from(e),
            AccountError::ClassHashCalculation(e) => TransactionError::ComputeClassHash(e),
            AccountError::ClassCompression(e) => TransactionError::ClassCompression(e),
            AccountError::FeeOutOfRange => TransactionError::FeeOutOfRange,
        }
    }
}

impl<S> From<ProviderError> for TransactionError<S>
where
    S: std::error::Error,
{
    fn from(value: ProviderError) -> Self {
        println!("FROM: {:?}", value);
        match &value {
            ProviderError::StarknetError(e) => match &e {
                StarknetError::TransactionExecutionError(te) => {
                    TransactionError::TransactionExecution(te.execution_error.clone())
                }
                _ => TransactionError::Provider(value),
            },
            _ => TransactionError::Provider(value),
        }
    }
}
