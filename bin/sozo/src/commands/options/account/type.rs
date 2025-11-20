use std::sync::Arc;

use async_trait::async_trait;
#[cfg(feature = "controller")]
use slot::account_sdk::provider::CartridgeJsonRpcProvider;
use starknet::accounts::{
    single_owner, Account, ConnectedAccount, ExecutionEncoder, RawDeclarationV3, RawExecutionV3,
    SingleOwnerAccount,
};
use starknet::core::types::{BlockId, Call, Felt};
use starknet::providers::Provider;
use starknet::signers::{local_wallet, LocalWallet, SignerInteractivityContext};

#[cfg(feature = "controller")]
use super::controller::ControllerAccount;
#[cfg(feature = "controller")]
use super::provider::EitherProvider;

#[cfg(feature = "controller")]
type RpcProvider<P> = EitherProvider<Arc<P>, CartridgeJsonRpcProvider>;
#[cfg(not(feature = "controller"))]
type RpcProvider<P> = Arc<P>;

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
#[allow(missing_debug_implementations)]
pub enum SozoAccountKind<P>
where
    P: Provider + Send + Sync,
{
    Standard(SingleOwnerAccount<Arc<P>, LocalWallet>),
    #[cfg(feature = "controller")]
    Controller(Box<ControllerAccount>),
}

pub struct SozoAccount<P>
where
    P: Provider + Send + Sync,
{
    account: SozoAccountKind<P>,
    provider: RpcProvider<P>,
}

impl<P> SozoAccount<P>
where
    P: Provider + Send + Sync,
{
    pub fn new_standard(
        provider: Arc<P>,
        account: SingleOwnerAccount<Arc<P>, LocalWallet>,
    ) -> Self {
        let account = SozoAccountKind::Standard(account);
        #[cfg(feature = "controller")]
        let provider = EitherProvider::Left(provider);
        #[cfg(not(feature = "controller"))]
        let provider = provider;
        Self { account, provider }
    }

    #[cfg(feature = "controller")]
    pub fn new_controller(
        provider: CartridgeJsonRpcProvider,
        controller: ControllerAccount,
    ) -> Self {
        let account = SozoAccountKind::Controller(Box::new(controller));
        let provider = EitherProvider::Right(provider);
        Self { account, provider }
    }
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
        match &self.account {
            SozoAccountKind::Standard(account) => account.is_signer_interactive(context),
            #[cfg(feature = "controller")]
            SozoAccountKind::Controller(account) => account.is_signer_interactive(context),
        }
    }

    fn address(&self) -> Felt {
        match &self.account {
            SozoAccountKind::Standard(account) => account.address(),
            #[cfg(feature = "controller")]
            SozoAccountKind::Controller(account) => account.address(),
        }
    }

    fn chain_id(&self) -> Felt {
        match &self.account {
            SozoAccountKind::Standard(account) => account.chain_id(),
            #[cfg(feature = "controller")]
            SozoAccountKind::Controller(account) => account.chain_id(),
        }
    }

    async fn sign_execution_v3(
        &self,
        execution: &RawExecutionV3,
        query_only: bool,
    ) -> Result<Vec<Felt>, Self::SignError> {
        let result = match &self.account {
            SozoAccountKind::Standard(account) => {
                account.sign_execution_v3(execution, query_only).await?
            }
            #[cfg(feature = "controller")]
            SozoAccountKind::Controller(account) => {
                account.sign_execution_v3(execution, query_only).await?
            }
        };
        Ok(result)
    }

    async fn sign_declaration_v3(
        &self,
        declaration: &RawDeclarationV3,
        query_only: bool,
    ) -> Result<Vec<Felt>, Self::SignError> {
        let result = match &self.account {
            SozoAccountKind::Standard(account) => {
                account.sign_declaration_v3(declaration, query_only).await?
            }
            #[cfg(feature = "controller")]
            SozoAccountKind::Controller(account) => {
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
        match &self.account {
            SozoAccountKind::Standard(account) => account.encode_calls(calls),
            #[cfg(feature = "controller")]
            SozoAccountKind::Controller(account) => account.encode_calls(calls),
        }
    }
}

impl<P> ConnectedAccount for SozoAccount<P>
where
    P: Provider,
    P: Send + Sync,
{
    type Provider = RpcProvider<P>;

    fn provider(&self) -> &Self::Provider {
        &self.provider
    }

    fn block_id(&self) -> BlockId {
        match &self.account {
            SozoAccountKind::Standard(account) => account.block_id(),
            #[cfg(feature = "controller")]
            SozoAccountKind::Controller(account) => account.block_id(),
        }
    }
}
