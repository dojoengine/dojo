//! TODO: Add module documentation.

mod ethereum;
mod service;
mod starknet;

use std::path::Path;

use ::starknet::core::types::MsgToL1;
use ::starknet::providers::jsonrpc::HttpTransport;
use ::starknet::providers::{JsonRpcClient, Provider};
use anyhow::Result;
use async_trait::async_trait;
use ethereum::EthereumMessaging;
use ethers::providers::ProviderError as EthereumProviderError;
use serde::Deserialize;
use tracing::{error, info};

pub use self::service::{MessagingOutcome, MessagingService};
use self::starknet::StarknetMessaging;

pub(crate) const LOG_TARGET: &str = "messaging";

type MessengerResult<T> = Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to initialize messaging")]
    InitError,
    #[error("Failed to gather messages")]
    GatherError,
    #[error("Failed to send messages")]
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
    async fn gather_messages(
        &self,
        from_block: u64,
        max_blocks: u64,
    ) -> MessengerResult<(u64, Vec<Self::MessageTransaction>)>;

    /// Computes the hash of the given messages and sends them to the settlement chain.
    ///
    /// Once message's hash is settled, one must send a transaction (with the message content)
    /// on the settlement chain to actually consume it.
    ///
    /// # Arguments
    ///
    /// * `messages` - Messages to settle.
    async fn settle_messages(
        &self,
        messages: &[MsgToL1],
    ) -> MessengerResult<Vec<Self::MessageHash>>;
}

pub enum MessengerMode {
    Ethereum(EthereumMessaging),
    Starknet(StarknetMessaging),
}

impl MessengerMode {
    pub async fn from_config(config: MessagingConfig) -> MessengerResult<Self> {
        if config.contract_address.len() < 50 {
            match EthereumMessaging::new(config).await {
                Ok(m_eth) => {
                    info!(target: LOG_TARGET, "Messaging enabled [Ethereum]");
                    Ok(MessengerMode::Ethereum(m_eth))
                }
                Err(e) => {
                    error!(target: LOG_TARGET, "Ethereum messenger init failed: {e}");
                    Err(Error::InitError)
                }
            }
        } else {
            match StarknetMessaging::new(config).await {
                Ok(m_sn) => {
                    info!(target: LOG_TARGET, "Messaging enabled [Starknet]");
                    Ok(MessengerMode::Starknet(m_sn))
                }
                Err(e) => {
                    error!(target: LOG_TARGET, "Starknet messenger init failed: {e}");
                    Err(Error::InitError)
                }
            }
        }
    }
}
