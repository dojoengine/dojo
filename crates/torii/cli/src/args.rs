use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use dojo_utils::parse::parse_url;
use merge_options::MergeOptions;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use url::Url;

use super::options::*;

pub const DEFAULT_RPC_URL: &str = "http://0.0.0.0:5050";

/// Dojo World Indexer
#[derive(Parser, Debug, Serialize, Deserialize, Clone, MergeOptions)]
#[serde(default)]
#[command(name = "torii", author, about, long_about = None)]
#[command(next_help_heading = "Torii general options")]
pub struct ToriiArgs {
    /// The world to index
    #[arg(short, long = "world", env = "DOJO_WORLD_ADDRESS")]
    pub world_address: Option<Felt>,

    /// The sequencer rpc endpoint to index.
    #[arg(long, value_name = "URL", default_value = DEFAULT_RPC_URL, value_parser = parse_url)]
    pub rpc: Url,

    /// Database filepath (ex: indexer.db). If specified file doesn't exist, it will be
    /// created. Defaults to in-memory database.
    #[arg(long)]
    #[arg(
        value_name = "PATH",
        help = "Database filepath. If specified directory doesn't exist, it will be created. \
                Defaults to in-memory database."
    )]
    pub db_dir: Option<PathBuf>,

    /// Configuration file
    #[arg(long, help = "Configuration file to setup Torii.")]
    pub config: Option<PathBuf>,

    /// Optional path to dump config to
    #[arg(long, help = "Optional path to dump config to")]
    pub dump_config: Option<PathBuf>,

    #[command(flatten)]
    pub runner: RunnerOptions,

    #[command(flatten)]
    pub indexing: IndexingOptions,

    #[command(flatten)]
    pub events: EventsOptions,

    #[command(flatten)]
    pub erc: ErcOptions,

    #[command(flatten)]
    pub sql: SqlOptions,

    #[cfg(feature = "server")]
    #[command(flatten)]
    pub metrics: MetricsOptions,

    #[cfg(feature = "server")]
    #[command(flatten)]
    pub server: ServerOptions,

    #[cfg(feature = "server")]
    #[command(flatten)]
    pub relay: RelayOptions,
}

impl Default for ToriiArgs {
    fn default() -> Self {
        Self {
            world_address: None,
            rpc: Url::parse(DEFAULT_RPC_URL).unwrap(),
            db_dir: None,
            config: None,
            dump_config: None,
            indexing: IndexingOptions::default(),
            events: EventsOptions::default(),
            erc: ErcOptions::default(),
            sql: SqlOptions::default(),
            runner: RunnerOptions::default(),
            #[cfg(feature = "server")]
            metrics: MetricsOptions::default(),
            #[cfg(feature = "server")]
            server: ServerOptions::default(),
            #[cfg(feature = "server")]
            relay: RelayOptions::default(),
        }
    }
}

impl ToriiArgs {
    pub fn with_config_file(mut self) -> Result<Self> {
        let config: Self = if let Some(path) = &self.config {
            toml::from_str(&std::fs::read_to_string(path)?)?
        } else {
            return Ok(self);
        };

        // the CLI (self) takes precedence over the config file.
        self.merge(Some(&config));

        Ok(self)
    }
}

#[cfg(test)]
mod test {
    use std::net::{IpAddr, Ipv4Addr};
    use std::str::FromStr;

    use torii_sqlite_types::{Contract, ContractType, ModelIndices};

    use super::*;

    #[test]
    fn test_cli_precedence() {
        // CLI args must take precedence over the config file.
        let content = r#"
        world_address = "0x1234"
        rpc = "http://0.0.0.0:5050"
        db_dir = "/tmp/torii-test"

        [indexing]
        transactions = false

        [sql]
        historical = [
            "ns-E",
            "ns-EH"
        ]
        page_size = 2048

        [[sql.model_indices]]
        model_tag = "ns-Position"
        fields = ["vec.x", "vec.y"]
        
        [events]
        raw = true
        "#;
        let path = std::env::temp_dir().join("torii-config2.json");
        std::fs::write(&path, content).unwrap();

        let path_str = path.to_string_lossy().to_string();

        let args = vec![
            "torii",
            "--world",
            "0x9999",
            "--rpc",
            "http://0.0.0.0:6060",
            "--db-dir",
            "/tmp/torii-test2",
            "--sql.historical",
            "a-A",
            "--indexing.transactions",
            "--sql.model_indices",
            "ns-Position:vec.x,vec.y;ns-Moves:player",
            "--sql.page_size",
            "1024",
            "--config",
            path_str.as_str(),
        ];

        let torii_args = ToriiArgs::parse_from(args).with_config_file().unwrap();

        assert_eq!(torii_args.world_address, Some(Felt::from_str("0x9999").unwrap()));
        assert_eq!(torii_args.rpc, Url::parse("http://0.0.0.0:6060").unwrap());
        assert_eq!(torii_args.db_dir, Some(PathBuf::from("/tmp/torii-test2")));
        assert!(torii_args.events.raw);
        assert_eq!(torii_args.sql.historical, vec!["a-A".to_string()]);
        assert_eq!(torii_args.server, ServerOptions::default());
        assert!(torii_args.indexing.transactions);
        assert_eq!(torii_args.sql.page_size, 1024);
        assert_eq!(torii_args.sql.cache_size, DEFAULT_DATABASE_CACHE_SIZE);
        assert_eq!(
            torii_args.sql.model_indices,
            Some(vec![
                ModelIndices {
                    model_tag: "ns-Position".to_string(),
                    fields: vec!["vec.x".to_string(), "vec.y".to_string()],
                },
                ModelIndices {
                    model_tag: "ns-Moves".to_string(),
                    fields: vec!["player".to_string()],
                },
            ])
        );
    }

    #[test]
    fn test_config_default_empty_toml() {
        // give empty cli args and an empty toml file and check it has the default values
        let content = "";
        let path = std::env::temp_dir().join("torii-config3.json");
        std::fs::write(&path, content).unwrap();

        let path_str = path.to_string_lossy().to_string();

        let args = vec!["torii", "--config", path_str.as_str()];

        let torii_args = ToriiArgs::parse_from(args).with_config_file().unwrap();

        assert_eq!(torii_args.world_address, None);

        assert_eq!(torii_args.rpc, Url::parse(DEFAULT_RPC_URL).unwrap());

        assert_eq!(torii_args.db_dir, None);

        assert_eq!(torii_args.indexing, IndexingOptions::default());
        assert_eq!(torii_args.events, EventsOptions::default());
        assert_eq!(torii_args.erc, ErcOptions::default());
        assert_eq!(torii_args.sql, SqlOptions::default());
        assert_eq!(torii_args.runner, RunnerOptions::default());
        assert_eq!(torii_args.server, ServerOptions::default());
        assert_eq!(torii_args.relay, RelayOptions::default());
        assert_eq!(torii_args.metrics, MetricsOptions::default());

        assert_eq!(torii_args.indexing.blocks_chunk_size, DEFAULT_BLOCKS_CHUNK_SIZE);
        assert_eq!(torii_args.indexing.events_chunk_size, DEFAULT_EVENTS_CHUNK_SIZE);
        assert!(torii_args.indexing.pending);
        assert_eq!(torii_args.indexing.polling_interval, DEFAULT_POLLING_INTERVAL);
        assert_eq!(torii_args.indexing.max_concurrent_tasks, DEFAULT_MAX_CONCURRENT_TASKS);

        assert!(!torii_args.events.raw);

        assert_eq!(torii_args.erc.max_metadata_tasks, DEFAULT_ERC_MAX_METADATA_TASKS);
        assert_eq!(torii_args.erc.artifacts_path, None);

        assert_eq!(torii_args.sql.page_size, DEFAULT_DATABASE_PAGE_SIZE);
        assert_eq!(torii_args.sql.cache_size, DEFAULT_DATABASE_CACHE_SIZE);
        assert_eq!(torii_args.sql.model_indices, None);
        assert_eq!(torii_args.sql.historical, Vec::<String>::new());

        assert_eq!(torii_args.server.http_addr, DEFAULT_HTTP_ADDR);
        assert_eq!(torii_args.server.http_port, DEFAULT_HTTP_PORT);
        assert_eq!(torii_args.server.http_cors_origins, None);

        assert!(!torii_args.metrics.metrics);
        assert_eq!(torii_args.metrics.metrics_addr, DEFAULT_METRICS_ADDR);
        assert_eq!(torii_args.metrics.metrics_port, DEFAULT_METRICS_PORT);

        assert_eq!(torii_args.relay.port, DEFAULT_RELAY_PORT);
        assert_eq!(torii_args.relay.webrtc_port, DEFAULT_RELAY_WEBRTC_PORT);
        assert_eq!(torii_args.relay.websocket_port, DEFAULT_RELAY_WEBSOCKET_PORT);
        assert_eq!(torii_args.relay.local_key_path, None);
        assert_eq!(torii_args.relay.cert_path, None);
        assert_eq!(torii_args.relay.peers, Vec::<String>::new());
    }

    #[test]
    fn test_config_fallback() {
        let content = r#"
        world_address = "0x1234"
        rpc = "http://0.0.0.0:2222"
        db_dir = "/tmp/torii-test"

        [events]
        raw = true

        [server]
        http_addr = "127.0.0.1"
        http_port = 7777
        http_cors_origins = ["*"]

        [indexing]
        events_chunk_size = 9999
        pending = true
        max_concurrent_tasks = 1000
        transactions = false
        contracts = [
            "erc20:0x1234",
            "erc721:0x5678"
        ]
        namespaces = []

        [sql]
        historical = [
            "ns-E",
            "ns-EH"
        ]

        [[sql.model_indices]]
        model_tag = "ns-Position"
        fields = ["vec.x", "vec.y"]
        "#;
        let path = std::env::temp_dir().join("torii-config.json");
        std::fs::write(&path, content).unwrap();

        let path_str = path.to_string_lossy().to_string();

        let args = vec!["torii", "--config", path_str.as_str()];

        let torii_args = ToriiArgs::parse_from(args).with_config_file().unwrap();

        assert_eq!(torii_args.world_address, Some(Felt::from_str("0x1234").unwrap()));
        assert_eq!(torii_args.rpc, Url::parse("http://0.0.0.0:2222").unwrap());
        assert_eq!(torii_args.db_dir, Some(PathBuf::from("/tmp/torii-test")));
        assert!(torii_args.events.raw);
        assert_eq!(torii_args.sql.historical, vec!["ns-E".to_string(), "ns-EH".to_string()]);
        assert_eq!(torii_args.indexing.events_chunk_size, 9999);
        assert_eq!(torii_args.indexing.blocks_chunk_size, 10240);
        assert!(torii_args.indexing.pending);
        assert_eq!(torii_args.indexing.polling_interval, 500);
        assert_eq!(torii_args.indexing.max_concurrent_tasks, 1000);
        assert!(!torii_args.indexing.transactions);
        assert_eq!(
            torii_args.indexing.contracts,
            vec![
                Contract {
                    address: Felt::from_str("0x1234").unwrap(),
                    r#type: ContractType::ERC20
                },
                Contract {
                    address: Felt::from_str("0x5678").unwrap(),
                    r#type: ContractType::ERC721
                }
            ]
        );
        assert_eq!(
            torii_args.sql.model_indices,
            Some(vec![ModelIndices {
                model_tag: "ns-Position".to_string(),
                fields: vec!["vec.x".to_string(), "vec.y".to_string()],
            }])
        );
        assert_eq!(torii_args.server.http_addr, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(torii_args.server.http_port, 7777);
        assert_eq!(torii_args.server.http_cors_origins, Some(vec!["*".to_string()]));
    }
}
