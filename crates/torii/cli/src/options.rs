use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use anyhow::Context;
use camino::Utf8PathBuf;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use torii_sqlite::types::{Contract, ContractType};

pub const DEFAULT_HTTP_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
pub const DEFAULT_HTTP_PORT: u16 = 8080;
pub const DEFAULT_METRICS_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
pub const DEFAULT_METRICS_PORT: u16 = 9200;
pub const DEFAULT_EVENTS_CHUNK_SIZE: u64 = 1024;
pub const DEFAULT_BLOCKS_CHUNK_SIZE: u64 = 10240;
pub const DEFAULT_POLLING_INTERVAL: u64 = 500;
pub const DEFAULT_MAX_CONCURRENT_TASKS: usize = 100;
pub const DEFAULT_RELAY_PORT: u16 = 9090;
pub const DEFAULT_RELAY_WEBRTC_PORT: u16 = 9091;
pub const DEFAULT_RELAY_WEBSOCKET_PORT: u16 = 9092;

pub const DEFAULT_ERC_MAX_METADATA_TASKS: usize = 10;

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
    #[serde(default)]
    pub local_key_path: Option<String>,

    /// Path to a local certificate file. If not specified, a new certificate will be generated
    /// for WebRTC connections
    #[arg(
        long = "relay.cert_path",
        value_name = "PATH",
        help = "Path to a local certificate file. If not specified, a new certificate will be \
                generated for WebRTC connections."
    )]
    #[serde(default)]
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
    #[arg(
        long = "indexing.pending",
        default_value_t = true,
        help = "Whether or not to index pending blocks."
    )]
    #[serde(default)]
    pub pending: bool,

    /// Polling interval in ms
    #[arg(
        long = "indexing.polling_interval",
        default_value_t = DEFAULT_POLLING_INTERVAL,
        help = "Polling interval in ms for Torii to check for new events."
    )]
    #[serde(default = "default_polling_interval")]
    pub polling_interval: u64,

    /// Maximum number of concurrent tasks used for processing parallelizable events.
    #[arg(
        long = "indexing.max_concurrent_tasks",
        default_value_t = DEFAULT_MAX_CONCURRENT_TASKS,
        help = "Maximum number of concurrent tasks processing parallelizable events."
    )]
    #[serde(default = "default_max_concurrent_tasks")]
    pub max_concurrent_tasks: usize,

    /// Whether or not to index world transactions
    #[arg(
        long = "indexing.transactions",
        default_value_t = false,
        help = "Whether or not to index world transactions and keep them in the database."
    )]
    #[serde(default)]
    pub transactions: bool,

    /// ERC contract addresses to index
    #[arg(
        long = "indexing.contracts",
        value_delimiter = ',',
        value_parser = parse_erc_contract,
        help = "ERC contract addresses to index. You may only specify ERC20 or ERC721 contracts."
    )]
    #[serde(deserialize_with = "deserialize_contracts")]
    #[serde(serialize_with = "serialize_contracts")]
    #[serde(default)]
    pub contracts: Vec<Contract>,

    /// Namespaces to index
    #[arg(
        long = "indexing.namespaces",
        value_delimiter = ',',
        help = "The namespaces of the world that torii should index. If empty, all namespaces \
                will be indexed."
    )]
    #[serde(default)]
    pub namespaces: Vec<String>,

    /// The block number to start indexing the world from.
    ///
    /// Warning: In the current implementation, this will break the indexing of tokens, if any.
    /// Since the tokens require the chain to be indexed from the beginning, to ensure correct
    /// balance updates.
    #[arg(
        long = "indexing.world_block",
        help = "The block number to start indexing from.",
        default_value_t = 0
    )]
    #[serde(default)]
    pub world_block: u64,

    /// Whether or not to index Cartridge controllers.
    #[arg(
        long = "indexing.controllers",
        default_value_t = false,
        help = "Whether or not to index Cartridge controllers."
    )]
    #[serde(default)]
    pub controllers: bool,

    /// Whether or not to read models from the block number they were registered in.
    /// If false, models will be read from the latest block.
    #[arg(
        long = "indexing.strict_model_reader",
        default_value_t = false,
        help = "Whether or not to read models from the block number they were registered in."
    )]
    #[serde(default)]
    pub strict_model_reader: bool,
}

impl Default for IndexingOptions {
    fn default() -> Self {
        Self {
            events_chunk_size: DEFAULT_EVENTS_CHUNK_SIZE,
            blocks_chunk_size: DEFAULT_BLOCKS_CHUNK_SIZE,
            pending: true,
            transactions: false,
            contracts: vec![],
            polling_interval: DEFAULT_POLLING_INTERVAL,
            max_concurrent_tasks: DEFAULT_MAX_CONCURRENT_TASKS,
            namespaces: vec![],
            world_block: 0,
            controllers: false,
            strict_model_reader: false,
        }
    }
}

impl IndexingOptions {
    pub fn merge(&mut self, other: Option<&Self>) {
        if let Some(other) = other {
            if self.events_chunk_size == DEFAULT_EVENTS_CHUNK_SIZE {
                self.events_chunk_size = other.events_chunk_size;
            }

            if self.blocks_chunk_size == DEFAULT_BLOCKS_CHUNK_SIZE {
                self.blocks_chunk_size = other.blocks_chunk_size;
            }

            if !self.pending {
                self.pending = other.pending;
            }

            if self.polling_interval == DEFAULT_POLLING_INTERVAL {
                self.polling_interval = other.polling_interval;
            }

            if self.max_concurrent_tasks == DEFAULT_MAX_CONCURRENT_TASKS {
                self.max_concurrent_tasks = other.max_concurrent_tasks;
            }

            if !self.transactions {
                self.transactions = other.transactions;
            }

            if self.contracts.is_empty() {
                self.contracts = other.contracts.clone();
            }

            if self.namespaces.is_empty() {
                self.namespaces = other.namespaces.clone();
            }

            if self.world_block == 0 {
                self.world_block = other.world_block;
            }

            if !self.controllers {
                self.controllers = other.controllers;
            }

            if !self.strict_model_reader {
                self.strict_model_reader = other.strict_model_reader;
            }
        }
    }
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, Default)]
#[command(next_help_heading = "Events indexing options")]
pub struct EventsOptions {
    /// Whether or not to index raw events
    #[arg(
        long = "events.raw",
        default_value_t = false,
        help = "Whether or not to index raw events."
    )]
    #[serde(default)]
    pub raw: bool,

    /// Event messages that are going to be treated as historical
    /// A list of the model tags (namespace-name)
    #[arg(
        long = "events.historical",
        value_delimiter = ',',
        help = "Event messages that are going to be treated as historical during indexing."
    )]
    #[serde(default)]
    pub historical: Vec<String>,
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
    #[serde(default)]
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
    #[serde(default)]
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

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "ERC options")]
pub struct ErcOptions {
    /// The maximum number of concurrent tasks to use for indexing ERC721 and ERC1155 token
    /// metadata.
    #[arg(
        long = "erc.max_metadata_tasks",
        default_value_t = DEFAULT_ERC_MAX_METADATA_TASKS,
        help = "The maximum number of concurrent tasks to use for indexing ERC721 and ERC1155 token metadata."
    )]
    #[serde(default = "default_erc_max_metadata_tasks")]
    pub max_metadata_tasks: usize,

    /// Path to a directory to store ERC artifacts
    #[arg(long)]
    pub artifacts_path: Option<Utf8PathBuf>,
}

impl Default for ErcOptions {
    fn default() -> Self {
        Self { max_metadata_tasks: DEFAULT_ERC_MAX_METADATA_TASKS, artifacts_path: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelIndices {
    pub model_tag: String,
    pub fields: Vec<String>,
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "SQL options")]
pub struct SqlOptions {
    /// Whether model tables should default to having indices on all columns
    #[arg(
        long = "sql.model_indices_keys",
        default_value_t = false,
        help = "If true, creates indices on only key fields columns of model tables by default. If false, all model field columns will have indices."
    )]
    #[serde(default)]
    pub model_indices_keys: bool,

    /// Specify which fields should have indices for specific models
    /// Format: "model_name:field1,field2;another_model:field3,field4"
    #[arg(
        long = "sql.model_indices",
        value_delimiter = ';',
        value_parser = parse_model_indices,
        help = "Specify which fields should have indices for specific models. Format: \"model_name:field1,field2;another_model:field3,field4\""
    )]
    #[serde(default)]
    pub model_indices: Option<Vec<ModelIndices>>,
}

impl Default for SqlOptions {
    fn default() -> Self {
        Self { model_indices_keys: false, model_indices: None }
    }
}

// Parses clap cli argument which is expected to be in the format:
// - model-tag:field1,field2;othermodel-tag:field3,field4
fn parse_model_indices(part: &str) -> anyhow::Result<ModelIndices> {
    let parts = part.split(':').collect::<Vec<&str>>();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid model indices format"));
    }

    let model_tag = parts[0].to_string();
    let fields = parts[1].split(',').map(|s| s.to_string()).collect::<Vec<_>>();

    Ok(ModelIndices { model_tag, fields })
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

fn serialize_contracts<S>(contracts: &Vec<Contract>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut seq = serializer.serialize_seq(Some(contracts.len()))?;

    for contract in contracts {
        seq.serialize_element(&contract.to_string())?;
    }

    seq.end()
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

fn default_erc_max_metadata_tasks() -> usize {
    DEFAULT_ERC_MAX_METADATA_TASKS
}
