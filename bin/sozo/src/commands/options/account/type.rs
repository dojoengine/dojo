use std::sync::Arc;

use async_trait::async_trait;
use starknet::accounts::{
    single_owner, Account, ConnectedAccount, DeclarationV2, DeclarationV3, ExecutionEncoder,
    ExecutionV1, ExecutionV3, LegacyDeclaration, RawDeclarationV2, RawDeclarationV3,
    RawExecutionV1, RawExecutionV3, RawLegacyDeclaration, SingleOwnerAccount,
};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::{BlockId, Call, Felt, FlattenedSierraClass};
use starknet::providers::Provider;
use starknet::signers::{local_wallet, LocalWallet, SignerInteractivityContext};

#[cfg(feature = "controller")]
use super::controller::ControllerSessionAccount;

#[derive(Debug, thiserror::Error)]
pub enum SozoAccountSignError {
    #[error(transparent)]
    Standard(#[from] single_owner::SignError<local_wallet::SignError>),

    #[cfg(feature = "controller")]
    #[error(transparent)]
    Controller(#[from] slot::account_sdk::signers::SignError),
}

/// To unify the account types, we define a wrapper type that implements the
/// [ConnectedAccount] trait and wrap the different account types.
///
/// This is the account type that should be used by the CLI.
#[must_use]
#[non_exhaustive]
#[allow(missing_debug_implementations)]
#[allow(clippy::large_enum_variant)]
pub enum SozoAccount<P>
where
    P: Provider + Send + Sync,
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

    fn is_signer_interactive(&self, context: SignerInteractivityContext<'_>) -> bool {
        match self {
            Self::Standard(account) => account.is_signer_interactive(context),
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.is_signer_interactive(context),
        }
    }

    fn address(&self) -> Felt {
        match self {
            Self::Standard(account) => account.address(),
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.address(),
        }
    }

    fn chain_id(&self) -> Felt {
        match self {
            Self::Standard(account) => account.chain_id(),
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.chain_id(),
        }
    }

    fn declare_legacy(
        &self,
        contract_class: Arc<LegacyContractClass>,
    ) -> LegacyDeclaration<'_, Self> {
        LegacyDeclaration::new(contract_class, self)
    }

    fn declare(
        &self,
        contract_class: Arc<FlattenedSierraClass>,
        compiled_class_hash: Felt,
    ) -> DeclarationV2<'_, Self> {
        DeclarationV2::new(contract_class, compiled_class_hash, self)
    }

    fn declare_v2(
        &self,
        contract_class: Arc<FlattenedSierraClass>,
        compiled_class_hash: Felt,
    ) -> DeclarationV2<'_, Self> {
        DeclarationV2::new(contract_class, compiled_class_hash, self)
    }

    fn declare_v3(
        &self,
        contract_class: Arc<FlattenedSierraClass>,
        compiled_class_hash: Felt,
    ) -> DeclarationV3<'_, Self> {
        DeclarationV3::new(contract_class, compiled_class_hash, self)
    }

    fn execute(&self, calls: Vec<Call>) -> ExecutionV1<'_, Self> {
        ExecutionV1::new(calls, self)
    }

    fn execute_v1(&self, calls: Vec<Call>) -> ExecutionV1<'_, Self> {
        ExecutionV1::new(calls, self)
    }

    fn execute_v3(&self, calls: Vec<Call>) -> ExecutionV3<'_, Self> {
        ExecutionV3::new(calls, self)
    }

    async fn sign_execution_v1(
        &self,
        execution: &RawExecutionV1,
        query_only: bool,
    ) -> Result<Vec<Felt>, Self::SignError> {
        let result = match self {
            Self::Standard(account) => account.sign_execution_v1(execution, query_only).await?,
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.sign_execution_v1(execution, query_only).await?,
        };
        Ok(result)
    }

    async fn sign_execution_v3(
        &self,
        execution: &RawExecutionV3,
        query_only: bool,
    ) -> Result<Vec<Felt>, Self::SignError> {
        let result = match self {
            Self::Standard(account) => account.sign_execution_v3(execution, query_only).await?,
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.sign_execution_v3(execution, query_only).await?,
        };
        Ok(result)
    }

    async fn sign_legacy_declaration(
        &self,
        declaration: &RawLegacyDeclaration,
        query_only: bool,
    ) -> Result<Vec<Felt>, Self::SignError> {
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

    async fn sign_declaration_v2(
        &self,
        declaration: &RawDeclarationV2,
        query_only: bool,
    ) -> Result<Vec<Felt>, Self::SignError> {
        let result = match self {
            Self::Standard(account) => account.sign_declaration_v2(declaration, query_only).await?,

            #[cfg(feature = "controller")]
            Self::Controller(account) => {
                account.sign_declaration_v2(declaration, query_only).await?
            }
        };
        Ok(result)
    }

    async fn sign_declaration_v3(
        &self,
        declaration: &RawDeclarationV3,
        query_only: bool,
    ) -> Result<Vec<Felt>, Self::SignError> {
        let result = match self {
            Self::Standard(account) => account.sign_declaration_v3(declaration, query_only).await?,

            #[cfg(feature = "controller")]
            Self::Controller(account) => {
                account.sign_declaration_v3(declaration, query_only).await?
            }
        };
        Ok(result)
    }
}

impl<P> ExecutionEncoder for SozoAccount<P>
where
    P: Provider,
    P: Send + Sync,
{
    fn encode_calls(&self, calls: &[Call]) -> Vec<Felt> {
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

    fn block_id(&self) -> BlockId {
        match self {
            Self::Standard(account) => account.block_id(),
            #[cfg(feature = "controller")]
            Self::Controller(account) => account.block_id(),
        }
    }
}
