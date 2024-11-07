pub mod declarer;
pub mod deployer;
pub mod error;
pub mod invoker;
pub mod waiter;

use std::fmt;

use anyhow::{anyhow, Result};
use colored_json::ToColoredJson;
use reqwest::Url;
use starknet::accounts::{
    AccountDeploymentV1, AccountDeploymentV3, AccountError, AccountFactory, AccountFactoryError,
    ConnectedAccount, DeclarationV2, DeclarationV3, ExecutionEncoding, ExecutionV1, ExecutionV3,
    SingleOwnerAccount,
};
use starknet::core::types::{
    BlockId, BlockTag, DeclareTransactionResult, DeployAccountTransactionResult, Felt,
    InvokeTransactionResult, TransactionReceiptWithBlockInfo,
};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{AnyProvider, JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};

#[derive(Debug, Copy, Clone, Default)]
pub struct StrkFeeConfig {
    /// The maximum L1 gas amount.
    pub gas: Option<u64>,
    /// The maximum L1 gas price in STRK.
    pub gas_price: Option<u128>,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct EthFeeConfig {
    /// The multiplier for how much the actual transaction max fee should be relative to the
    /// estimated fee.
    pub fee_estimate_multiplier: Option<f64>,
    /// The maximum fee to pay for the transaction.
    pub max_fee_raw: Option<Felt>,
}

#[derive(Debug, Copy, Clone)]
pub enum FeeConfig {
    /// The STRK fee configuration.
    Strk(StrkFeeConfig),
    /// The ETH fee configuration.
    Eth(EthFeeConfig),
}

impl Default for FeeConfig {
    fn default() -> Self {
        Self::Strk(StrkFeeConfig::default())
    }
}

/// The transaction configuration to use when sending a transaction.
#[derive(Debug, Copy, Clone, Default)]
pub struct TxnConfig {
    /// Whether to wait for the transaction to be accepted or reverted on L2.
    pub wait: bool,
    /// Whether to display the transaction receipt.
    pub receipt: bool,
    /// Whether to use the `walnut` fee estimation strategy.
    pub walnut: bool,
    /// The fee configuration to use for the transaction.
    pub fee_config: FeeConfig,
}

#[derive(Debug, Clone)]
pub enum TxnAction {
    Send { wait: bool, receipt: bool, fee_config: FeeConfig, walnut: bool },
    Estimate,
    Simulate,
}

#[derive(Debug)]
pub enum TransactionResult {
    /// In some occasions, the transaction is not sent and it's not an error.
    /// Typically for the deployer/declarer/invoker that have internal logic to check if the
    /// transaction is needed or not.
    Noop,
    /// The transaction hash.
    Hash(Felt),
    /// The transaction hash and it's receipt.
    HashReceipt(Felt, Box<TransactionReceiptWithBlockInfo>),
}

impl fmt::Display for TransactionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionResult::Hash(hash) => write!(f, "Transaction hash: {:#066x}", hash),
            TransactionResult::HashReceipt(hash, receipt) => write!(
                f,
                "Transaction hash: {:#066x}\nReceipt: {}",
                hash,
                serde_json::to_string_pretty(&receipt)
                    .expect("Failed to serialize receipt")
                    .to_colored_json_auto()
                    .expect("Failed to colorize receipt")
            ),
            TransactionResult::Noop => write!(f, "Transaction was not sent"),
        }
    }
}

impl TxnConfig {
    pub fn init_wait() -> Self {
        Self { wait: true, ..Default::default() }
    }
}

/// Helper trait to abstract away setting `TxnConfig` configurations before sending a transaction
/// Implemented by types from `starknet-accounts` like `Execution`, `Declaration`, etc...
#[allow(async_fn_in_trait)]
pub trait TransactionExt<T> {
    type R;
    type U;

    /// Sets `fee_estimate_multiplier` and `max_fee_raw` from `TxnConfig` if its present before
    /// calling `send` method on the respective type.
    /// NOTE: If both are specified `max_fee_raw` will take precedence and `fee_estimate_multiplier`
    /// will be ignored by `starknet-rs`
    async fn send_with_cfg(self, txn_config: &TxnConfig) -> Result<Self::R, Self::U>;
}

impl<T> TransactionExt<T> for ExecutionV1<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = InvokeTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        if let FeeConfig::Eth(c) = txn_config.fee_config {
            if let Some(fee_est_mul) = c.fee_estimate_multiplier {
                self = self.fee_estimate_multiplier(fee_est_mul);
            }

            if let Some(max_raw_f) = c.max_fee_raw {
                self = self.max_fee(max_raw_f);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExt<T> for DeclarationV2<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = DeclareTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        if let FeeConfig::Eth(c) = txn_config.fee_config {
            if let Some(fee_est_mul) = c.fee_estimate_multiplier {
                self = self.fee_estimate_multiplier(fee_est_mul);
            }

            if let Some(max_raw_f) = c.max_fee_raw {
                self = self.max_fee(max_raw_f);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExt<T> for AccountDeploymentV1<'_, T>
where
    T: AccountFactory + Sync,
{
    type R = DeployAccountTransactionResult;
    type U = AccountFactoryError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountFactoryError<<T>::SignError>> {
        if let FeeConfig::Eth(c) = txn_config.fee_config {
            if let Some(fee_est_mul) = c.fee_estimate_multiplier {
                self = self.fee_estimate_multiplier(fee_est_mul);
            }

            if let Some(max_raw_f) = c.max_fee_raw {
                self = self.max_fee(max_raw_f);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExt<T> for ExecutionV3<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = InvokeTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        if let FeeConfig::Strk(c) = txn_config.fee_config {
            if let Some(g) = c.gas {
                self = self.gas(g);
            }

            if let Some(gp) = c.gas_price {
                self = self.gas_price(gp);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExt<T> for DeclarationV3<'_, T>
where
    T: ConnectedAccount + Sync,
{
    type R = DeclareTransactionResult;
    type U = AccountError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountError<T::SignError>> {
        if let FeeConfig::Strk(c) = txn_config.fee_config {
            if let Some(g) = c.gas {
                self = self.gas(g);
            }

            if let Some(gp) = c.gas_price {
                self = self.gas_price(gp);
            }
        }

        self.send().await
    }
}

impl<T> TransactionExt<T> for AccountDeploymentV3<'_, T>
where
    T: AccountFactory + Sync,
{
    type R = DeployAccountTransactionResult;
    type U = AccountFactoryError<T::SignError>;

    async fn send_with_cfg(
        mut self,
        txn_config: &TxnConfig,
    ) -> Result<Self::R, AccountFactoryError<<T>::SignError>> {
        if let FeeConfig::Strk(c) = txn_config.fee_config {
            if let Some(g) = c.gas {
                self = self.gas(g);
            }

            if let Some(gp) = c.gas_price {
                self = self.gas_price(gp);
            }
        }

        self.send().await
    }
}

/// Parses a string into a [`BlockId`].
///
/// # Arguments
///
/// * `block_str` - a string representing a block ID. It could be a block hash starting with 0x, a
///   block number, 'pending' or 'latest'.
///
/// # Returns
///
/// The parsed [`BlockId`] on success.
pub fn parse_block_id(block_str: String) -> Result<BlockId> {
    if block_str.starts_with("0x") {
        let hash = Felt::from_hex(&block_str)
            .map_err(|_| anyhow!("Unable to parse block hash: {}", block_str))?;
        Ok(BlockId::Hash(hash))
    } else if block_str.eq("pending") {
        Ok(BlockId::Tag(BlockTag::Pending))
    } else if block_str.eq("latest") {
        Ok(BlockId::Tag(BlockTag::Latest))
    } else {
        match block_str.parse::<u64>() {
            Ok(n) => Ok(BlockId::Number(n)),
            Err(_) => Err(anyhow!("Unable to parse block ID: {}", block_str)),
        }
    }
}

/// Get predeployed accounts from the RPC provider.
pub async fn get_predeployed_accounts<A: ConnectedAccount>(
    migrator: A,
    rpc_url: &str,
) -> anyhow::Result<Vec<SingleOwnerAccount<AnyProvider, LocalWallet>>> {
    let client = reqwest::Client::new();
    let response = client
        .post(rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "dev_predeployedAccounts",
            "params": [],
            "id": 1
        }))
        .send()
        .await;

    if response.is_err() {
        return Ok(vec![]);
    }

    let result: serde_json::Value = response.unwrap().json().await?;

    let mut declarers = vec![];

    if let Some(vals) = result.get("result").and_then(|v| v.as_array()) {
        let chain_id = migrator.provider().chain_id().await?;

        for a in vals {
            let address = a["address"].as_str().unwrap();

            // On slot, some accounts are hidden, we skip them.
            let private_key = if let Some(pk) = a["privateKey"].as_str() {
                pk
            } else {
                continue;
            };

            let provider = AnyProvider::JsonRpcHttp(JsonRpcClient::new(HttpTransport::new(
                Url::parse(rpc_url).unwrap(),
            )));

            let signer = LocalWallet::from(SigningKey::from_secret_scalar(
                Felt::from_hex(private_key).unwrap(),
            ));

            let mut account = SingleOwnerAccount::new(
                provider,
                signer,
                Felt::from_hex(address).unwrap(),
                chain_id,
                ExecutionEncoding::New,
            );

            account.set_block_id(BlockId::Tag(BlockTag::Pending));

            declarers.push(account);
        }
    }

    Ok(declarers)
}
