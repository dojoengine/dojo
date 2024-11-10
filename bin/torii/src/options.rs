use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use anyhow::Context;
use clap::ArgAction;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use torii_core::types::{Contract, ContractType};

const DEFAULT_HTTP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_HTTP_PORT: u16 = 8080;
const DEFAULT_METRICS_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const DEFAULT_METRICS_PORT: u16 = 9200;
const DEFAULT_EVENTS_CHUNK_SIZE: u64 = 1024;
const DEFAULT_BLOCKS_CHUNK_SIZE: u64 = 10240;
const DEFAULT_POLLING_INTERVAL: u64 = 500;
const DEFAULT_MAX_CONCURRENT_TASKS: usize = 100;

const DEFAULT_RELAY_PORT: u16 = 9090;
const DEFAULT_RELAY_WEBRTC_PORT: u16 = 9091;
const DEFAULT_RELAY_WEBSOCKET_PORT: u16 = 9092;

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "Relay options")]
pub struct RelayOptions {
    /// Port to serve Libp2p TCP & UDP Quic transports
    #[arg(
        long = "relay.port",
        value_name = "PORT",
        default_value_t = DEFAULT_RELAY_PORT,
        help = "Port to serve Libp2p TCP & UDP Quic transports."
    )]
    #[serde(default = "default_relay_port")]
    pub port: u16,

    /// Port to serve Libp2p WebRTC transport
    #[arg(
        long = "relay.webrtc_port",
        value_name = "PORT",
        default_value_t = DEFAULT_RELAY_WEBRTC_PORT,
        help = "Port to serve Libp2p WebRTC transport."
    )]
    #[serde(default = "default_relay_webrtc_port")]
    pub webrtc_port: u16,

    /// Port to serve Libp2p WebRTC transport
    #[arg(
        long = "relay.websocket_port",
        value_name = "PORT",
        default_value_t = DEFAULT_RELAY_WEBSOCKET_PORT,
        help = "Port to serve Libp2p WebRTC transport."
    )]
    #[serde(default = "default_relay_websocket_port")]
    pub websocket_port: u16,

    /// Path to a local identity key file. If not specified, a new identity will be generated
    #[arg(
        long = "relay.local_key_path",
        value_name = "PATH",
        help = "Path to a local identity key file. If not specified, a new identity will be \
                generated."
    )]
    pub local_key_path: Option<String>,

    /// Path to a local certificate file. If not specified, a new certificate will be generated
    /// for WebRTC connections
    #[arg(
        long = "relay.cert_path",
        value_name = "PATH",
        help = "Path to a local certificate file. If not specified, a new certificate will be \
                generated for WebRTC connections."
    )]
    pub cert_path: Option<String>,
}

impl Default for RelayOptions {
    fn default() -> Self {
        Self {
            port: DEFAULT_RELAY_PORT,
            webrtc_port: DEFAULT_RELAY_WEBRTC_PORT,
            websocket_port: DEFAULT_RELAY_WEBSOCKET_PORT,
            local_key_path: None,
            cert_path: None,
        }
    }
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "Indexing options")]
pub struct IndexingOptions {
    /// Chunk size of the events page when indexing using events
    #[arg(long = "indexing.events_chunk_size", default_value_t = DEFAULT_EVENTS_CHUNK_SIZE, help = "Chunk size of the events page to fetch from the sequencer.")]
    #[serde(default = "default_events_chunk_size")]
    pub events_chunk_size: u64,

    /// Number of blocks to process before commiting to DB
    #[arg(long = "indexing.blocks_chunk_size", default_value_t = DEFAULT_BLOCKS_CHUNK_SIZE, help = "Number of blocks to process before commiting to DB.")]
    #[serde(default = "default_blocks_chunk_size")]
    pub blocks_chunk_size: u64,

    /// Enable indexing pending blocks
    #[arg(long = "indexing.pending", action = ArgAction::Set, default_value_t = true, help = "Whether or not to index pending blocks.")]
    pub index_pending: bool,

    /// Polling interval in ms
    #[arg(
        long = "indexing.polling_interval",
        default_value_t = DEFAULT_POLLING_INTERVAL,
        help = "Polling interval in ms for Torii to check for new events."
    )]
    #[serde(default = "default_polling_interval")]
    pub polling_interval: u64,

    /// Max concurrent tasks
    #[arg(
        long = "indexing.max_concurrent_tasks",
        default_value_t = DEFAULT_MAX_CONCURRENT_TASKS,
        help = "Max concurrent tasks used to parallelize indexing."
    )]
    #[serde(default = "default_max_concurrent_tasks")]
    pub max_concurrent_tasks: usize,

    /// Whether or not to index world transactions
    #[arg(
        long = "indexing.transactions",
        action = ArgAction::Set,
        default_value_t = false,
        help = "Whether or not to index world transactions and keep them in the database."
    )]
    pub index_transactions: bool,

    /// ERC contract addresses to index
    #[arg(
        long = "indexing.contracts",
        value_delimiter = ',',
        value_parser = parse_erc_contract,
        help = "ERC contract addresses to index. You may only specify ERC20 or ERC721 contracts."
    )]
    #[serde(deserialize_with = "deserialize_contracts")]
    pub contracts: Vec<Contract>,
}

impl Default for IndexingOptions {
    fn default() -> Self {
        Self {
            events_chunk_size: DEFAULT_EVENTS_CHUNK_SIZE,
            blocks_chunk_size: DEFAULT_BLOCKS_CHUNK_SIZE,
            index_pending: true,
            index_transactions: false,
            contracts: vec![],
            polling_interval: DEFAULT_POLLING_INTERVAL,
            max_concurrent_tasks: DEFAULT_MAX_CONCURRENT_TASKS,
        }
    }
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "Events indexing options")]
pub struct EventsOptions {
    /// Whether or not to index raw events
    #[arg(long = "events.raw", action = ArgAction::Set, default_value_t = true, help = "Whether or not to index raw events.")]
    pub raw: bool,

    /// Event messages that are going to be treated as historical
    /// A list of the model tags (namespace-name)
    #[arg(
        long = "events.historical",
        value_delimiter = ',',
        help = "Event messages that are going to be treated as historical during indexing."
    )]
    pub historical: Option<Vec<String>>,
}

impl Default for EventsOptions {
    fn default() -> Self {
        Self { raw: true, historical: None }
    }
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "HTTP server options")]
pub struct ServerOptions {
    /// HTTP server listening interface.
    #[arg(long = "http.addr", value_name = "ADDRESS")]
    #[arg(default_value_t = DEFAULT_HTTP_ADDR)]
    #[serde(default = "default_http_addr")]
    pub http_addr: IpAddr,

    /// HTTP server listening port.
    #[arg(long = "http.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_HTTP_PORT)]
    #[serde(default = "default_http_port")]
    pub http_port: u16,

    /// Comma separated list of domains from which to accept cross origin requests.
    #[arg(long = "http.cors_origins")]
    #[arg(value_delimiter = ',')]
    pub http_cors_origins: Option<Vec<String>>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self { http_addr: DEFAULT_HTTP_ADDR, http_port: DEFAULT_HTTP_PORT, http_cors_origins: None }
    }
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "Metrics options")]
pub struct MetricsOptions {
    /// Enable metrics.
    ///
    /// For now, metrics will still be collected even if this flag is not set. This only
    /// controls whether the metrics server is started or not.
    #[arg(long)]
    pub metrics: bool,

    /// The metrics will be served at the given address.
    #[arg(requires = "metrics")]
    #[arg(long = "metrics.addr", value_name = "ADDRESS")]
    #[arg(default_value_t = DEFAULT_METRICS_ADDR)]
    #[serde(default = "default_metrics_addr")]
    pub metrics_addr: IpAddr,

    /// The metrics will be served at the given port.
    #[arg(requires = "metrics")]
    #[arg(long = "metrics.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_METRICS_PORT)]
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

impl Default for MetricsOptions {
    fn default() -> Self {
        Self {
            metrics: false,
            metrics_addr: DEFAULT_METRICS_ADDR,
            metrics_port: DEFAULT_METRICS_PORT,
        }
    }
}

// Parses clap cli argument which is expected to be in the format:
// - erc_type:address:start_block
// - address:start_block (erc_type defaults to ERC20)
fn parse_erc_contract(part: &str) -> anyhow::Result<Contract> {
    match part.split(':').collect::<Vec<&str>>().as_slice() {
        [r#type, address] => {
            let r#type = r#type.parse::<ContractType>()?;
            if r#type == ContractType::WORLD {
                return Err(anyhow::anyhow!(
                    "World address cannot be specified as an ERC contract"
                ));
            }

            let address = Felt::from_str(address)
                .with_context(|| format!("Expected address, found {}", address))?;
            Ok(Contract { address, r#type })
        }
        _ => Err(anyhow::anyhow!("Invalid contract format")),
    }
}

// Add this function to handle TOML deserialization
fn deserialize_contracts<'de, D>(deserializer: D) -> Result<Vec<Contract>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let contracts: Vec<String> = Vec::deserialize(deserializer)?;
    contracts.iter().map(|s| parse_erc_contract(s).map_err(serde::de::Error::custom)).collect()
}

// ** Default functions to setup serde of the configuration file **
fn default_http_addr() -> IpAddr {
    DEFAULT_HTTP_ADDR
}

fn default_http_port() -> u16 {
    DEFAULT_HTTP_PORT
}

fn default_metrics_addr() -> IpAddr {
    DEFAULT_METRICS_ADDR
}

fn default_metrics_port() -> u16 {
    DEFAULT_METRICS_PORT
}

fn default_events_chunk_size() -> u64 {
    DEFAULT_EVENTS_CHUNK_SIZE
}

fn default_blocks_chunk_size() -> u64 {
    DEFAULT_BLOCKS_CHUNK_SIZE
}

fn default_polling_interval() -> u64 {
    DEFAULT_POLLING_INTERVAL
}

fn default_max_concurrent_tasks() -> usize {
    DEFAULT_MAX_CONCURRENT_TASKS
}

fn default_relay_port() -> u16 {
    DEFAULT_RELAY_PORT
}

fn default_relay_webrtc_port() -> u16 {
    DEFAULT_RELAY_WEBRTC_PORT
}

fn default_relay_websocket_port() -> u16 {
    DEFAULT_RELAY_WEBSOCKET_PORT
}
