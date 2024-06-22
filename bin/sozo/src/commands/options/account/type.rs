use std::sync::Arc;

use async_trait::async_trait;
use starknet::accounts::single_owner::SignError;
use starknet::accounts::{
    Account, Call, ConnectedAccount, Declaration, Execution, ExecutionEncoder, LegacyDeclaration,
    RawDeclaration, RawExecution, RawLegacyDeclaration, SingleOwnerAccount,
};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::{FieldElement, FlattenedSierraClass};
use starknet::providers::Provider;
use starknet::signers::LocalWallet;

#[cfg(feature = "controller")]
use super::controller::ControllerSessionAccount;

#[derive(Debug, thiserror::Error)]
pub enum SozoAccountSignError {
    #[error(transparent)]
    Standard(#[from] SignError<starknet::signers::local_wallet::SignError>),

    #[cfg(feature = "controller")]
    #[error(transparent)]
    Controller(#[from] account_sdk::signers::SignError),
}

/// To unify the account types, we define a wrapper type that implements the
/// [ConnectedAccount] trait and wrap the different account types.
///
/// This is the account type that should be used by the CLI.
#[must_use]
#[non_exhaustive]
#[derive(derive_more::From)]
pub enum SozoAccount<P>
where
    P: Send,
    P: Provider,
{
    Standard(SingleOwnerAccount<P, LocalWallet>),

    #[cfg(feature = "controller")]
    Controller(ControllerSessionAccount<P>),
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<P> Account for SozoAccount<P>
where
    P: Provider,
    P: Send + Sync,
{
    type SignError = SozoAccountSignError;

    fn address(&self) -> FieldElement {
        match self {
            Self::Standard(account) => account.address(),
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.address(),
        }
    }

    fn chain_id(&self) -> FieldElement {
        match self {
            Self::Standard(account) => account.chain_id(),
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.chain_id(),
        }
    }

    fn declare(
        &self,
        contract_class: Arc<FlattenedSierraClass>,
        compiled_class_hash: FieldElement,
    ) -> Declaration<'_, Self> {
        Declaration::new(contract_class, compiled_class_hash, self)
    }

    fn declare_legacy(
        &self,
        contract_class: Arc<LegacyContractClass>,
    ) -> LegacyDeclaration<'_, Self> {
        LegacyDeclaration::new(contract_class, self)
    }

    fn execute(&self, calls: Vec<Call>) -> Execution<'_, Self> {
        Execution::new(calls, self)
    }

    async fn sign_execution(
        &self,
        execution: &RawExecution,
        query_only: bool,
    ) -> Result<Vec<FieldElement>, Self::SignError> {
        let result = match self {
            Self::Standard(account) => account.sign_execution(execution, query_only).await?,
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.sign_execution(execution, query_only).await?,
        };
        Ok(result)
    }

    async fn sign_declaration(
        &self,
        declaration: &RawDeclaration,
        query_only: bool,
    ) -> Result<Vec<FieldElement>, Self::SignError> {
        let result = match self {
            Self::Standard(account) => account.sign_declaration(declaration, query_only).await?,
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.sign_declaration(declaration, query_only).await?,
        };
        Ok(result)
    }

    async fn sign_legacy_declaration(
        &self,
        declaration: &RawLegacyDeclaration,
        query_only: bool,
    ) -> Result<Vec<FieldElement>, Self::SignError> {
        match self {
            Self::Standard(account) => {
                let result = account.sign_legacy_declaration(declaration, query_only).await?;
                Ok(result)
            }
            #[cfg(feature = "controller")]
            Self::Controller(account) => {
                let result = account.sign_legacy_declaration(declaration, query_only).await?;
                Ok(result)
            }
        }
    }
}

impl<P> ExecutionEncoder for SozoAccount<P>
where
    P: Provider,
    P: Send + Sync,
{
    fn encode_calls(&self, calls: &[Call]) -> Vec<FieldElement> {
        match self {
            Self::Standard(account) => account.encode_calls(calls),
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.encode_calls(calls),
        }
    }
}

impl<P> ConnectedAccount for SozoAccount<P>
where
    P: Provider,
    P: Send + Sync,
{
    type Provider = P;

    fn provider(&self) -> &Self::Provider {
        match self {
            Self::Standard(account) => account.provider(),
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.provider(),
        }
    }
}
