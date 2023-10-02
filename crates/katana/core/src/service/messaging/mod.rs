//! Messaging module.
//!
//! Messaging is the capability of a sequencer to gather/send messages from/to a settlement chain.
//! By default, the messaging feature of Katana uses Ethereum as settlement chain.
//! This feature is useful to locally test the interaction of Katana used as a Starknet dev node,
//! and third party Ethereum dev node like Anvil.
//!
//! The gathering is done by fetching logs from the settlement chain to then self execute a
//! `L1HandlerTransaction`. There is no account involved to execute this transaction, fees are
//! charged on the settlement layer.
//!
//! The sending of the messages is realized by collecting all the `messages_sent` from local
//! execution of smart contracts using the `send_message_to_l1_syscall`. Once messages are
//! collected, the hash of each message is computed and then registered on the settlement layer to
//! be consumed on the latter (by manually sending a transaction on the settlement chain). The
//! hashes are registered using a custom contract that mimics the verification of Starknet state
//! updates on Ethereum, since the process of proving and verifying of state updates, and then
//! posting in on the settlement layer are not yet present in Katana.
//!
//! Katana also has a `starknet-messaging` feature, where an opiniated implementation of L2 <-> L3
//! messaging is implemented using Starknet as settlement chain.
//!
//! With this feature, Katana also has the capability to directly send `invoke` transactions to a
//! Starknet contract. This is usually used in the L2 <-> L3 messaging configuration, to circumvent
//! the manual consumption of the message.
//!
//! In this module, the messaging service clearly separates the two implementations for each
//! settlement chain configuration in `starknet.rs` and `ethereum.rs`. The `service.rs` file aims at
//! running the common logic.
//!
//! To start Katana with the messaging enabled, the option `--messaging` must be used with a
//! configuration file following the `MessagingConfig` format. An example of this file can be found
//! in the messaging contracts.

mod ethereum;
mod service;
#[cfg(feature = "starknet-messaging")]
mod starknet;

use std::path::Path;

use ::starknet::core::types::{FieldElement, MsgToL1};
use ::starknet::providers::jsonrpc::HttpTransport;
use ::starknet::providers::{JsonRpcClient, Provider};
use anyhow::Result;
use async_trait::async_trait;
use ethereum::EthereumMessaging;
use ethers::providers::ProviderError as EthereumProviderError;
use serde::Deserialize;
use tracing::{error, info};

pub use self::service::{MessagingOutcome, MessagingService};
#[cfg(feature = "starknet-messaging")]
use self::starknet::StarknetMessaging;

pub(crate) const LOG_TARGET: &str = "messaging";
pub(crate) const CONFIG_CHAIN_ETHEREUM: &str = "ethereum";
#[cfg(feature = "starknet-messaging")]
pub(crate) const CONFIG_CHAIN_STARKNET: &str = "starknet";

type MessengerResult<T> = Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to initialize messaging")]
    InitError,
    #[error("Unsupported settlement chain")]
    UnsupportedChain,
    #[error("Failed to gather messages from settlement chain")]
    GatherError,
    #[error("Failed to send messages to settlement chain")]
    SendError,
    #[error(transparent)]
    Provider(ProviderError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Ethereum provider error: {0}")]
    Ethereum(EthereumProviderError),
    #[error("Starknet provider error: {0}")]
    Starknet(<JsonRpcClient<HttpTransport> as Provider>::Error),
}

impl From<EthereumProviderError> for Error {
    fn from(e: EthereumProviderError) -> Self {
        Self::Provider(ProviderError::Ethereum(e))
    }
}

/// The config used to initialize the messaging service.
#[derive(Debug, Default, Deserialize, Clone)]
pub struct MessagingConfig {
    /// The settlement chain.
    pub chain: String,
    /// The RPC-URL of the settlement chain.
    pub rpc_url: String,
    /// The messaging-contract address on the settlement chain.
    pub contract_address: String,
    /// The address to use for settling messages. It should be a valid address that
    /// can be used to initiate a transaction on the settlement chain.
    pub sender_address: String,
    /// The private key associated to `sender_address`.
    pub private_key: String,
    /// The interval, in seconds, at which the messaging service will fetch and settle messages
    /// from/to the settlement chain.
    pub interval: u64,
    /// The block on settlement chain from where Katana will start fetching messages.
    pub from_block: u64,
}

impl MessagingConfig {
    /// Load the config from a JSON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let buf = std::fs::read(path)?;
        serde_json::from_slice(&buf).map_err(|e| e.into())
    }

    /// This is used as the clap `value_parser` implementation
    pub fn parse(path: &str) -> Result<Self, String> {
        Self::load(path).map_err(|e| e.to_string())
    }
}

#[async_trait]
pub trait Messenger {
    /// The type of the message hash.
    type MessageHash;
    /// The transaction type of the message after being collected from the settlement chain.
    /// This is the transaction type that the message will be converted to before being added to the
    /// transaction pool.
    type MessageTransaction;

    /// Gathers messages emitted on the settlement chain and convert them to their
    /// corresponding transaction type on Starknet, and the latest block on the settlement until
    /// which the messages were collected.
    ///
    /// # Arguments
    ///
    /// * `from_block` - From which block the messages should be gathered.
    /// * `max_block` - The number of block fetched in the event/log filter. A too big value can
    ///   cause the RPC node to reject the query.
    /// * `chain_id` - The sequencer chain id for transaction hash computation.
    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64,
        chain_id: FieldElement,
    ) -> MessengerResult<(u64, Vec<Self::MessageTransaction>)>;

    /// Computes the hash of the given messages and sends them to the settlement chain.
    ///
    /// Once message's hash is settled, one must send a transaction (with the message content)
    /// on the settlement chain to actually consume it.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to settle.
    async fn send_messages(&self, messages: &[MsgToL1]) -> MessengerResult<Vec<Self::MessageHash>>;
}

pub enum MessengerMode {
    Ethereum(EthereumMessaging),
    #[cfg(feature = "starknet-messaging")]
    Starknet(StarknetMessaging),
}

impl MessengerMode {
    pub async fn from_config(config: MessagingConfig) -> MessengerResult<Self> {
        match config.chain.as_str() {
            CONFIG_CHAIN_ETHEREUM => match EthereumMessaging::new(config).await {
                Ok(m_eth) => {
                    info!(target: LOG_TARGET, "Messaging enabled [Ethereum]");
                    Ok(MessengerMode::Ethereum(m_eth))
                }
                Err(e) => {
                    error!(target: LOG_TARGET, "Ethereum messenger init failed: {e}");
                    Err(Error::InitError)
                }
            },

            #[cfg(feature = "starknet-messaging")]
            CONFIG_CHAIN_STARKNET => match StarknetMessaging::new(config).await {
                Ok(m_sn) => {
                    info!(target: LOG_TARGET, "Messaging enabled [Starknet]");
                    Ok(MessengerMode::Starknet(m_sn))
                }
                Err(e) => {
                    error!(target: LOG_TARGET, "Starknet messenger init failed: {e}");
                    Err(Error::InitError)
                }
            },

            chain => {
                error!(target: LOG_TARGET, "Unsupported settlement chain: {}", chain);
                Err(Error::UnsupportedChain)
            }
        }
    }
}
