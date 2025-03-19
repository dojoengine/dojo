#![cfg_attr(not(test), warn(unused_crate_dependencies))]

//! Messaging module.
//!
//! Messaging is the capability of a sequencer to gather messages from a settlement chain and
//! execute them, and send messages to a settlement chain via a provable mechanism (STARK proof in
//! the case of Starknet).
//!
//! By default, the messaging feature of Katana uses Ethereum as settlement chain.
//! This feature is useful to locally test the interaction of Katana used as a Starknet dev node,
//! and an Ethereum dev node like Anvil.
//!
//! The gathering is done by fetching logs from the settlement chain to then self execute a
//! `L1HandlerTransaction`. There is no account involved to execute this transaction, fees are
//! charged on the settlement layer.
//!
//! The sending of the messages is realized by collecting all the `messages_sent` from local
//! execution of smart contracts using the `send_message_to_l1_syscall`. Once the messages are
//! collected, the `StarknetOS` cairo program is executed to generate a proof of the produced block,
//! which contains the messages. This proof is then sent to the settlement chain where it is
//! verified, and the messages are consumed.
//!
//! Katana also has starknet messaging built-in, where an opiniated implementation of L2 <-> L3
//! messaging is implemented using Starknet as settlement chain.
//! When working with `L2 <> L3` with settlement on Starknet, there is one limitation:
//! Blockifier limits the `to_address` to be an EthAddress, which is smaller than the `Felt` type.
//! <https://github.com/starkware-libs/sequencer/blob/f4b25dd4689ba8ddec3c7db57ea7e8fd7ce32eab/crates/blockifier/src/execution/call_info.rs#L41>
//!
//! An applicative solution would be to use the `MSG` magic value as `to_address`, and the actual
//! `to_address` used as the first element of the `payload` of the message. This would also require
//! the settlement contract to be aware of this.
//!
//! In this module, the messaging service clearly separates the two implementations for each
//! settlement chain configuration in `starknet.rs` and `ethereum.rs`. The `service.rs` file aims at
//! running the common logic.

mod ethereum;
mod service;
mod starknet;

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use ::starknet::providers::ProviderError as StarknetProviderError;
use alloy_transport::TransportError;
use anyhow::Result;
use async_trait::async_trait;
use ethereum::EthereumMessaging;
use futures::StreamExt;
use katana_primitives::chain::ChainId;
use serde::{Deserialize, Serialize};
use tracing::{error, info, trace};

pub use self::service::{MessagingOutcome, MessagingService};
use self::starknet::StarknetMessaging;

pub(crate) const LOG_TARGET: &str = "messaging";
pub(crate) const CONFIG_CHAIN_ETHEREUM: &str = "ethereum";
pub(crate) const CONFIG_CHAIN_STARKNET: &str = "starknet";
pub(crate) const CONFIG_CHAIN_SOVEREIGN: &str = "sovereign";

type MessengerResult<T> = Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to initialize messaging")]
    InitError,
    #[error("Unsupported settlement chain")]
    UnsupportedChain,
    #[error("Failed to gather messages from settlement chain")]
    GatherError,
    #[error(transparent)]
    Provider(ProviderError),
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Ethereum provider error: {0}")]
    Ethereum(TransportError),
    #[error("Starknet provider error: {0}")]
    Starknet(StarknetProviderError),
}

impl From<TransportError> for Error {
    fn from(e: TransportError) -> Self {
        Self::Provider(ProviderError::Ethereum(e))
    }
}

/// The config used to initialize the messaging service.
#[derive(Debug, Default, Deserialize, Clone, Serialize, PartialEq)]
pub struct MessagingConfig {
    /// The settlement chain.
    pub chain: String,
    /// The RPC-URL of the settlement chain.
    pub rpc_url: String,
    /// The messaging-contract address on the settlement chain.
    pub contract_address: String,
    /// The interval, in seconds, at which the messaging service will fetch messages
    /// from the settlement chain.
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

    pub fn from_chain_spec(spec: &katana_chain_spec::rollup::ChainSpec) -> Self {
        match &spec.settlement {
            katana_chain_spec::SettlementLayer::Ethereum {
                rpc_url, core_contract, block, ..
            } => Self {
                chain: CONFIG_CHAIN_ETHEREUM.to_string(),
                rpc_url: rpc_url.to_string(),
                contract_address: core_contract.to_string(),
                from_block: *block,
                interval: 2,
            },
            katana_chain_spec::SettlementLayer::Starknet {
                rpc_url, core_contract, block, ..
            } => Self {
                chain: CONFIG_CHAIN_STARKNET.to_string(),
                rpc_url: rpc_url.to_string(),
                contract_address: core_contract.to_string(),
                from_block: *block,
                interval: 2,
            },
            katana_chain_spec::SettlementLayer::Sovereign { .. } => Self {
                chain: CONFIG_CHAIN_SOVEREIGN.to_string(),
                // Ideally, we don't want to trigger the await on the messaging service if in
                // sovereign mode.
                interval: 60,
                ..Default::default()
            },
        }
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
        chain_id: ChainId,
    ) -> MessengerResult<(u64, Vec<Self::MessageTransaction>)>;
}

#[derive(Debug)]
pub enum MessengerMode {
    Ethereum(EthereumMessaging),
    Starknet(StarknetMessaging),
    Sovereign(SovereignMessaging),
}

#[derive(Debug)]
pub struct SovereignMessaging {}

impl MessengerMode {
    pub async fn from_config(config: MessagingConfig) -> MessengerResult<Self> {
        match config.chain.as_str() {
            CONFIG_CHAIN_ETHEREUM => match EthereumMessaging::new(config).await {
                Ok(m_eth) => {
                    info!(target: LOG_TARGET, "Messaging enabled [Ethereum].");
                    Ok(MessengerMode::Ethereum(m_eth))
                }
                Err(e) => {
                    error!(target: LOG_TARGET,  error = %e, "Ethereum messenger init.");
                    Err(Error::InitError)
                }
            },

            CONFIG_CHAIN_STARKNET => match StarknetMessaging::new(config).await {
                Ok(m_sn) => {
                    info!(target: LOG_TARGET, "Messaging enabled [Starknet].");
                    Ok(MessengerMode::Starknet(m_sn))
                }
                Err(e) => {
                    error!(target: LOG_TARGET, error = %e, "Starknet messenger init.");
                    Err(Error::InitError)
                }
            },

            CONFIG_CHAIN_SOVEREIGN => {
                info!(target: LOG_TARGET, "Messaging not available [Sovereign].");
                Ok(MessengerMode::Sovereign(SovereignMessaging {}))
            }

            chain => {
                error!(target: LOG_TARGET, chain = %chain, "Unsupported settlement chain.");
                Err(Error::UnsupportedChain)
            }
        }
    }
}

#[allow(missing_debug_implementations)]
#[must_use = "MessagingTask does nothing unless polled"]
pub struct MessagingTask {
    messaging: MessagingService,
}

impl MessagingTask {
    pub fn new(messaging: MessagingService) -> Self {
        Self { messaging }
    }
}

impl Future for MessagingTask {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while let Poll::Ready(Some(outcome)) = this.messaging.poll_next_unpin(cx) {
            let MessagingOutcome { msg_count, .. } = outcome;
            {
                if msg_count > 0 {
                    info!(target: LOG_TARGET, %msg_count, "Collected messages from settlement chain.");
                }

                trace!(target: LOG_TARGET, %msg_count, "Collected messages from settlement chain.");
            }
        }

        Poll::Pending
    }
}
