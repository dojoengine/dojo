//! The migration related errors.

use dojo_utils::{TransactionError, TransactionWaitingError};
use starknet::core::types::FromStrError;
use starknet::core::utils::CairoShortStringToFeltError;
use starknet::providers::ProviderError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationError<S>
where
    S: std::error::Error,
{
    #[error(transparent)]
    CairoSerde(#[from] cainome::cairo_serde::Error),
    #[error(transparent)]
    Provider(ProviderError),
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
    #[error(transparent)]
    TransactionError(#[from] TransactionError<S>),
    #[error("Declaration of class failed: {0}")]
    DeclareClassError(String),
    #[error(transparent)]
    ContractVerificationError(anyhow::Error),
}
