pub mod waiter;

use std::fmt::{Display, Formatter};

use anyhow::Result;
use clap::builder::PossibleValue;
use clap::ValueEnum;
use starknet::accounts::{
    AccountDeploymentV1, AccountDeploymentV3, AccountError, AccountFactory, AccountFactoryError,
    ConnectedAccount, DeclarationV2, DeclarationV3, ExecutionV1, ExecutionV3,
};
use starknet::core::types::{
    Call, DeclareTransactionResult, DeployAccountTransactionResult, Felt, InvokeTransactionResult,
};

/// The transaction configuration to use when sending a transaction.
#[derive(Default, Debug, Copy, Clone)]
pub struct TxnConfig {
    pub wait: bool,
    pub receipt: bool,
    pub walnut: bool,
    pub fee_setting: FeeSetting,
}

impl TxnConfig {
    pub fn init_wait() -> Self {
        Self {
            wait: true,
            fee_setting: FeeSetting::Eth(TokenFeeSetting::Send(EthFeeSetting::Estimate {
                fee_estimate_multiplier: None,
            })),
            receipt: false,
            walnut: false,
        }
    }
}

/// Helper trait to abstract away setting `TxnConfig` configurations before sending a transaction
/// Implemented by types from `starknet-accounts` like `Execution`, `Declaration`, etc...
#[allow(async_fn_in_trait)]
pub trait TransactionExtETH<T> {
    type R;
    type U;

    /// Sets `fee_estimate_multiplier` and `max_fee_raw` from `TxnConfig` if its present before
    /// calling `send` method on the respective type.
    /// NOTE: If both are specified `max_fee_raw` will take precedence and `fee_estimate_multiplier`
    /// will be ignored by `starknet-rs`
    async fn send_with_cfg(self, txn_config: &EthFeeSetting) -> Result<Self::R, Self::U>;
}

#[allow(async_fn_in_trait)]
pub trait TransactionExtSTRK<T> {
    type R;
    type U;

    /// Sets `fee_estimate_multiplier` and `max_fee_raw` from `TxnConfig` if its present before
    /// calling `send` method on the respective type.
    /// NOTE: If both are specified `max_fee_raw` will take precedence and `fee_estimate_multiplier`
    /// will be ignored by `starknet-rs`
    async fn send_with_cfg(self, txn_config: &StrkFeeSetting) -> Result<Self::R, Self::U>;
}

impl<T> TransactionExtETH<T> for ExecutionV1<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = InvokeTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        fee_setting: &EthFeeSetting,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        match fee_setting {
            EthFeeSetting::Manual { max_fee_raw } => {
                self = self.max_fee(*max_fee_raw);
            }
            EthFeeSetting::Estimate { fee_estimate_multiplier } => {
                let fee_estimate_mul = fee_estimate_multiplier.unwrap_or(1.1);
                self = self.fee_estimate_multiplier(fee_estimate_mul);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExtSTRK<T> for ExecutionV3<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = InvokeTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        fee_setting: &StrkFeeSetting,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        match fee_setting {
            StrkFeeSetting::Manual { gas, gas_price } => {
                if let Some(gas) = gas {
                    self = self.gas(*gas);
                }

                if let Some(gas_price) = gas_price {
                    self = self.gas_price(*gas_price);
                }
            }
            StrkFeeSetting::Estimate { gas_estimate_multiplier } => {
                let gas_estimate_multiplier = gas_estimate_multiplier.unwrap_or(1.1);
                self = self.gas_estimate_multiplier(gas_estimate_multiplier);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExtETH<T> for DeclarationV2<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = DeclareTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        fee_setting: &EthFeeSetting,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        match fee_setting {
            EthFeeSetting::Manual { max_fee_raw } => {
                self = self.max_fee(*max_fee_raw);
            }
            EthFeeSetting::Estimate { fee_estimate_multiplier } => {
                let fee_estimate_mul = fee_estimate_multiplier.unwrap_or(1.1);
                self = self.fee_estimate_multiplier(fee_estimate_mul);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExtSTRK<T> for DeclarationV3<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = DeclareTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        fee_setting: &StrkFeeSetting,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        match fee_setting {
            StrkFeeSetting::Manual { gas, gas_price } => {
                if let Some(gas) = gas {
                    self = self.gas(*gas);
                }

                if let Some(gas_price) = gas_price {
                    self = self.gas_price(*gas_price);
                }
            }
            StrkFeeSetting::Estimate { gas_estimate_multiplier } => {
                let gas_estimate_multiplier = gas_estimate_multiplier.unwrap_or(1.1);
                self = self.gas_estimate_multiplier(gas_estimate_multiplier);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExtETH<T> for AccountDeploymentV1<'_, T>
where
    T: AccountFactory + Sync,
{
    type R = DeployAccountTransactionResult;
    type U = AccountFactoryError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        fee_setting: &EthFeeSetting,
    ) -> Result<Self::R, AccountFactoryError<<T>::SignError>> {
        match fee_setting {
            EthFeeSetting::Manual { max_fee_raw } => {
                self = self.max_fee(*max_fee_raw);
            }
            EthFeeSetting::Estimate { fee_estimate_multiplier } => {
                let fee_estimate_mul = fee_estimate_multiplier.unwrap_or(1.1);
                self = self.fee_estimate_multiplier(fee_estimate_mul);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExtSTRK<T> for AccountDeploymentV3<'_, T>
where
    T: AccountFactory + Sync,
{
    type R = DeployAccountTransactionResult;
    type U = AccountFactoryError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        fee_setting: &StrkFeeSetting,
    ) -> Result<Self::R, AccountFactoryError<<T>::SignError>> {
        match fee_setting {
            StrkFeeSetting::Manual { gas, gas_price } => {
                if let Some(gas) = gas {
                    self = self.gas(*gas);
                }

                if let Some(gas_price) = gas_price {
                    self = self.gas_price(*gas_price);
                }
            }
            StrkFeeSetting::Estimate { gas_estimate_multiplier } => {
                let gas_estimate_multiplier = gas_estimate_multiplier.unwrap_or(1.1);
                self = self.gas_estimate_multiplier(gas_estimate_multiplier);
            }
        }

        self.send().await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeToken {
    Eth,
    Strk,
}

#[derive(Debug, Copy, Clone)]
pub enum FeeSetting {
    Eth(TokenFeeSetting<EthFeeSetting>),
    Strk(TokenFeeSetting<StrkFeeSetting>),
}

impl Default for FeeSetting {
    fn default() -> Self {
        FeeSetting::Eth(TokenFeeSetting::Send(EthFeeSetting::Estimate {
            fee_estimate_multiplier: None,
        }))
    }
}

impl FeeSetting {
    pub fn fee_token(&self) -> FeeToken {
        match self {
            FeeSetting::Eth(_) => FeeToken::Eth,
            FeeSetting::Strk(_) => FeeToken::Strk,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum TokenFeeSetting<M> {
    Send(M),
    EstimateOnly,
    // TODO: simulate
}

impl<M> TokenFeeSetting<M> {
    pub fn is_estimate_only(&self) -> bool {
        matches!(self, Self::EstimateOnly)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum EthFeeSetting {
    Manual {
        max_fee_raw: Felt,
    },
    Estimate {
        /// if none, fee_estimate_multiplier is set to `1.1`.
        fee_estimate_multiplier: Option<f64>,
    },
}

#[derive(Debug, Copy, Clone)]
pub enum StrkFeeSetting {
    Manual {
        gas: Option<u64>,
        gas_price: Option<u128>,
    },
    Estimate {
        /// if none, fee_estimate_multiplier is set to `1.1`.
        gas_estimate_multiplier: Option<f64>,
    },
}

impl ValueEnum for FeeToken {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Eth, Self::Strk]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::Eth => Some(PossibleValue::new("ETH").alias("eth")),
            Self::Strk => Some(PossibleValue::new("STRK").alias("strk")),
        }
    }
}

impl Display for FeeToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eth => write!(f, "ETH"),
            Self::Strk => write!(f, "STRK"),
        }
    }
}

pub async fn handle_execute<A>(
    fee_setting: FeeSetting,
    account: &A,
    calls: Vec<Call>,
) -> Result<Option<InvokeTransactionResult>, AccountError<A::SignError>>
where
    A: ConnectedAccount + Sync,
{
    let invoke_res = match fee_setting {
        FeeSetting::Eth(token_fee_setting) => match token_fee_setting {
            TokenFeeSetting::Send(fee_setting) => {
                account.execute_v1(calls).send_with_cfg(&fee_setting).await?
            }
            TokenFeeSetting::EstimateOnly => todo!(),
        },
        FeeSetting::Strk(token_fee_setting) => match token_fee_setting {
            TokenFeeSetting::Send(fee_setting) => {
                account.execute_v3(calls).send_with_cfg(&fee_setting).await?
            }
            TokenFeeSetting::EstimateOnly => todo!(),
        },
    };

    Ok(Some(invoke_res))
}
