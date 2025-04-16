use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use anyhow::Context;
use camino::Utf8PathBuf;
use merge_options::MergeOptions;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use torii_sqlite_types::{Contract, ContractType, ModelIndices};

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

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
#[serde(default)]
#[command(next_help_heading = "Relay options")]
pub struct RelayOptions {
    /// Port to serve Libp2p TCP & UDP Quic transports
    #[arg(
        long = "relay.port",
        value_name = "PORT",
        default_value_t = DEFAULT_RELAY_PORT,
        help = "Port to serve Libp2p TCP & UDP Quic transports."
    )]
    pub port: u16,

    /// Port to serve Libp2p WebRTC transport
    #[arg(
        long = "relay.webrtc_port",
        value_name = "PORT",
        default_value_t = DEFAULT_RELAY_WEBRTC_PORT,
        help = "Port to serve Libp2p WebRTC transport."
    )]
    pub webrtc_port: u16,

    /// Port to serve Libp2p WebRTC transport
    #[arg(
        long = "relay.websocket_port",
        value_name = "PORT",
        default_value_t = DEFAULT_RELAY_WEBSOCKET_PORT,
        help = "Port to serve Libp2p WebRTC transport."
    )]
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

    /// A list of other torii relays to connect to and sync with.
    /// Right now, only offchain messages broadcasted by the relay will be synced.
    #[arg(
        long = "relay.peers",
        value_delimiter = ',',
        help = "A list of other torii relays to connect to and sync with."
    )]
    pub peers: Vec<String>,
}

impl Default for RelayOptions {
    fn default() -> Self {
        Self {
            port: DEFAULT_RELAY_PORT,
            webrtc_port: DEFAULT_RELAY_WEBRTC_PORT,
            websocket_port: DEFAULT_RELAY_WEBSOCKET_PORT,
            local_key_path: None,
            cert_path: None,
            peers: vec![],
        }
    }
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
#[serde(default)]
#[command(next_help_heading = "Indexing options")]
pub struct IndexingOptions {
    /// Chunk size of the events page when indexing using events
    #[arg(long = "indexing.events_chunk_size", default_value_t = DEFAULT_EVENTS_CHUNK_SIZE, help = "Chunk size of the events page to fetch from the sequencer.")]
    pub events_chunk_size: u64,

    /// Number of blocks to process before commiting to DB
    #[arg(long = "indexing.blocks_chunk_size", default_value_t = DEFAULT_BLOCKS_CHUNK_SIZE, help = "Number of blocks to process before commiting to DB.")]
    pub blocks_chunk_size: u64,

    /// Enable indexing pending blocks
    #[arg(
        long = "indexing.pending",
        default_value_t = true,
        help = "Whether or not to index pending blocks."
    )]
    pub pending: bool,

    /// Polling interval in ms
    #[arg(
        long = "indexing.polling_interval",
        default_value_t = DEFAULT_POLLING_INTERVAL,
        help = "Polling interval in ms for Torii to check for new events."
    )]
    pub polling_interval: u64,

    /// Maximum number of concurrent tasks used for processing parallelizable events.
    #[arg(
        long = "indexing.max_concurrent_tasks",
        default_value_t = DEFAULT_MAX_CONCURRENT_TASKS,
        help = "Maximum number of concurrent tasks processing parallelizable events."
    )]
    pub max_concurrent_tasks: usize,

    /// Whether or not to index world transactions
    #[arg(
        long = "indexing.transactions",
        default_value_t = false,
        help = "Whether or not to index world transactions and keep them in the database."
    )]
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
    pub contracts: Vec<Contract>,

    /// Namespaces to index
    #[arg(
        long = "indexing.namespaces",
        value_delimiter = ',',
        help = "The namespaces of the world that torii should index. If empty, all namespaces \
                will be indexed."
    )]
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
    pub world_block: u64,

    /// Whether or not to index Cartridge controllers.
    #[arg(
        long = "indexing.controllers",
        default_value_t = false,
        help = "Whether or not to index Cartridge controllers."
    )]
    pub controllers: bool,

    /// Whether or not to read models from the block number they were registered in.
    /// If false, models will be read from the latest block.
    #[arg(
        long = "indexing.strict_model_reader",
        default_value_t = false,
        help = "Whether or not to read models from the block number they were registered in."
    )]
    pub strict_model_reader: bool,
}

impl Default for IndexingOptions {
    fn default() -> Self {
        Self {
            events_chunk_size: DEFAULT_EVENTS_CHUNK_SIZE,
            blocks_chunk_size: DEFAULT_BLOCKS_CHUNK_SIZE,
            pending: true,
            polling_interval: DEFAULT_POLLING_INTERVAL,
            max_concurrent_tasks: DEFAULT_MAX_CONCURRENT_TASKS,
            transactions: false,
            contracts: vec![],
            namespaces: vec![],
            world_block: 0,
            controllers: false,
            strict_model_reader: false,
        }
    }
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, Default, MergeOptions)]
#[serde(default)]
#[command(next_help_heading = "Events indexing options")]
pub struct EventsOptions {
    /// Whether or not to index raw events
    #[arg(
        long = "events.raw",
        default_value_t = false,
        help = "Whether or not to index raw events."
    )]
    pub raw: bool,
}

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
#[serde(default)]
#[command(next_help_heading = "HTTP server options")]
pub struct ServerOptions {
    /// HTTP server listening interface.
    #[arg(long = "http.addr", value_name = "ADDRESS")]
    #[arg(default_value_t = DEFAULT_HTTP_ADDR)]
    pub http_addr: IpAddr,

    /// HTTP server listening port.
    #[arg(long = "http.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_HTTP_PORT)]
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

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
#[serde(default)]
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
    pub metrics_addr: IpAddr,

    /// The metrics will be served at the given port.
    #[arg(requires = "metrics")]
    #[arg(long = "metrics.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_METRICS_PORT)]
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

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
#[serde(default)]
#[command(next_help_heading = "ERC options")]
pub struct ErcOptions {
    /// The maximum number of concurrent tasks to use for indexing ERC721 and ERC1155 token
    /// metadata.
    #[arg(
        long = "erc.max_metadata_tasks",
        default_value_t = DEFAULT_ERC_MAX_METADATA_TASKS,
        help = "The maximum number of concurrent tasks to use for indexing ERC721 and ERC1155 token metadata."
    )]
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

pub const DEFAULT_DATABASE_PAGE_SIZE: u64 = 32_768;
/// Negative value is used to determine number of KiB to use for cache. Currently set as 512MB, 25%
/// of the RAM of the smallest slot instance.
pub const DEFAULT_DATABASE_CACHE_SIZE: i64 = -500_000;

#[derive(Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
#[serde(default)]
#[command(next_help_heading = "SQL options")]
pub struct SqlOptions {
    /// Whether model tables should default to having indices on all columns
    #[arg(
        long = "sql.all_model_indices",
        default_value_t = false,
        help = "If true, creates indices on all columns of model tables by default. If false, \
                only key fields columns of model tables will have indices."
    )]
    pub all_model_indices: bool,

    /// Specify which fields should have indices for specific models
    /// Format: "model_name:field1,field2;another_model:field3,field4"
    #[arg(
        long = "sql.model_indices",
        value_delimiter = ';',
        value_parser = parse_model_indices,
        help = "Specify which fields should have indices for specific models. Format: \"model_name:field1,field2;another_model:field3,field4\""
    )]
    pub model_indices: Option<Vec<ModelIndices>>,

    /// Models that are going to be treated as historical during indexing. Applies to event
    /// messages and entities. A list of the model tags (namespace-name)
    #[arg(
        long = "sql.historical",
        value_delimiter = ',',
        help = "Models that are going to be treated as historical during indexing."
    )]
    pub historical: Vec<String>,

    /// The page size to use for the database. The page size must be a power of two between 512 and
    /// 65536 inclusive.
    #[arg(
        long = "sql.page_size",
        default_value_t = DEFAULT_DATABASE_PAGE_SIZE,
        help = "The page size to use for the database. The page size must be a power of two between 512 and 65536 inclusive."
    )]
    pub page_size: u64,

    /// Cache size to use for the database.
    #[arg(
        long = "sql.cache_size",
        default_value_t = DEFAULT_DATABASE_CACHE_SIZE,
        help = "The cache size to use for the database. A positive value determines a number of pages, a negative value determines a number of KiB."
    )]
    pub cache_size: i64,
}

impl Default for SqlOptions {
    fn default() -> Self {
        Self {
            all_model_indices: false,
            model_indices: None,
            historical: vec![],
            page_size: DEFAULT_DATABASE_PAGE_SIZE,
            cache_size: DEFAULT_DATABASE_CACHE_SIZE,
        }
    }
}

#[derive(Default, Debug, clap::Args, Clone, Serialize, Deserialize, PartialEq, MergeOptions)]
#[serde(default)]
#[command(next_help_heading = "Runner options")]
pub struct RunnerOptions {
    /// Open World Explorer on the browser.
    #[arg(
        long = "runner.explorer",
        default_value_t = false,
        help = "Open World Explorer on the browser."
    )]
    pub explorer: bool,

    /// Check if contracts are deployed before starting torii.
    #[arg(
        long = "runner.check_contracts",
        default_value_t = false,
        help = "Check if contracts are deployed before starting torii."
    )]
    pub check_contracts: bool,
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
