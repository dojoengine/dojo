//! The migration related errors.

use dojo_utils::TransactionWaitingError;
use starknet::accounts::AccountError;
use starknet::core::types::contract::{CompressProgramError, ComputeClassHashError};
use starknet::core::types::{FromStrError, StarknetError};
use starknet::core::utils::CairoShortStringToFeltError;
use starknet::providers::ProviderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationError<S>
where
    S: std::error::Error,
{
    #[error(transparent)]
    SigningError(S),
    #[error(transparent)]
    CairoSerde(#[from] cainome::cairo_serde::Error),
    #[error(transparent)]
    ComputeClassHash(#[from] ComputeClassHashError),
    #[error(transparent)]
    ClassCompression(#[from] CompressProgramError),
    #[error("Fee calculation overflow")]
    FeeOutOfRange,
    #[error(transparent)]
    Provider(ProviderError),
    #[error("Transaction execution error: {0}")]
    TransactionExecution(String),
    #[error(transparent)]
    StarknetJson(#[from] starknet::core::types::contract::JsonError),
    #[error(
        "The contract `{0}` has no valid address, ensure this resource is known locally, or \
         remove it from the profile config writers/owners."
    )]
    OrphanSelectorAddress(String),
    #[error(transparent)]
    FromStr(#[from] FromStrError),
    #[error(transparent)]
    TransactionWaiting(#[from] TransactionWaitingError),
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
    #[error(
        "Failed to initialize contracts, verify the init call arguments in the profile config."
    )]
    InitCallArgs,
}

impl<S> From<AccountError<S>> for MigrationError<S>
where
    S: std::error::Error,
{
    fn from(value: AccountError<S>) -> Self {
        match value {
            AccountError::Signing(e) => MigrationError::SigningError(e),
            AccountError::Provider(e) => MigrationError::from(e),
            AccountError::ClassHashCalculation(e) => MigrationError::ComputeClassHash(e),
            AccountError::ClassCompression(e) => MigrationError::ClassCompression(e),
            AccountError::FeeOutOfRange => MigrationError::FeeOutOfRange,
        }
    }
}

impl<S> From<ProviderError> for MigrationError<S>
where
    S: std::error::Error,
{
    fn from(value: ProviderError) -> Self {
        match &value {
            ProviderError::StarknetError(e) => match &e {
                StarknetError::TransactionExecutionError(te) => {
                    MigrationError::TransactionExecution(te.execution_error.clone())
                }
                _ => MigrationError::Provider(value),
            },
            _ => MigrationError::Provider(value),
        }
    }
}
