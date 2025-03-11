use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use dojo_utils::parse::parse_url;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use url::Url;

use super::options::*;

pub const DEFAULT_RPC_URL: &str = "http://0.0.0.0:5050";

/// Dojo World Indexer
#[derive(Parser, Debug, Clone, serde::Serialize, serde::Deserialize)]
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

    /// Open World Explorer on the browser.
    #[arg(long, help = "Open World Explorer on the browser.")]
    pub explorer: bool,

    /// Configuration file
    #[arg(long, help = "Configuration file to setup Torii.")]
    pub config: Option<PathBuf>,

    #[command(flatten)]
    pub database: DatabaseOptions,

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

impl ToriiArgs {
    pub fn with_config_file(mut self) -> Result<Self> {
        let config: ToriiArgsConfig = if let Some(path) = &self.config {
            toml::from_str(&std::fs::read_to_string(path)?)?
        } else {
            return Ok(self);
        };

        // the CLI (self) takes precedence over the config file.
        // Currently, the merge is made at the top level of the commands.
        // We may add recursive merging in the future.

        if self.world_address.is_none() {
            self.world_address = config.world_address;
        }

        if self.rpc == Url::parse(DEFAULT_RPC_URL).unwrap() {
            if let Some(rpc) = config.rpc {
                self.rpc = rpc;
            }
        }

        if self.db_dir.is_none() {
            self.db_dir = config.db_dir;
        }

        // Currently the comparison it's only at the top level.
        // Need to make it more granular.

        if !self.explorer {
            self.explorer = config.explorer.unwrap_or_default();
        }

        self.database.merge(config.database.as_ref());

        self.indexing.merge(config.indexing.as_ref());

        if self.events == EventsOptions::default() {
            self.events = config.events.unwrap_or_default();
        }

        if self.erc == ErcOptions::default() {
            self.erc = config.erc.unwrap_or_default();
        }

        if self.sql == SqlOptions::default() {
            self.sql = config.sql.unwrap_or_default();
        }

        #[cfg(feature = "server")]
        {
            if self.server == ServerOptions::default() {
                self.server = config.server.unwrap_or_default();
            }

            if self.relay == RelayOptions::default() {
                self.relay = config.relay.unwrap_or_default();
            }

            if self.metrics == MetricsOptions::default() {
                self.metrics = config.metrics.unwrap_or_default();
            }
        }

        Ok(self)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ToriiArgsConfig {
    pub world_address: Option<Felt>,
    pub rpc: Option<Url>,
    pub db_dir: Option<PathBuf>,
    pub external_url: Option<Url>,
    pub explorer: Option<bool>,
    pub indexing: Option<IndexingOptions>,
    pub events: Option<EventsOptions>,
    pub database: Option<DatabaseOptions>,
    pub erc: Option<ErcOptions>,
    pub sql: Option<SqlOptions>,
    #[cfg(feature = "server")]
    pub metrics: Option<MetricsOptions>,
    #[cfg(feature = "server")]
    pub server: Option<ServerOptions>,
    #[cfg(feature = "server")]
    pub relay: Option<RelayOptions>,
}

#[cfg(test)]
mod test {
    use std::net::{IpAddr, Ipv4Addr};
    use std::str::FromStr;

    use torii_sqlite::types::{Contract, ContractType, ModelIndices};

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

        [events]
        raw = true
        historical = [
            "ns-E",
            "ns-EH"
        ]

        [database]
        page_size = 2048

        [[sql.model_indices]]
        model_tag = "ns-Position"
        fields = ["vec.x", "vec.y"]
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
            "--events.historical",
            "a-A",
            "--indexing.transactions",
            "--sql.model_indices",
            "ns-Position:vec.x,vec.y;ns-Moves:player",
            "--database.page_size",
            "1024",
            "--config",
            path_str.as_str(),
        ];

        let torii_args = ToriiArgs::parse_from(args).with_config_file().unwrap();

        assert_eq!(torii_args.world_address, Some(Felt::from_str("0x9999").unwrap()));
        assert_eq!(torii_args.rpc, Url::parse("http://0.0.0.0:6060").unwrap());
        assert_eq!(torii_args.db_dir, Some(PathBuf::from("/tmp/torii-test2")));
        assert!(!torii_args.events.raw);
        assert_eq!(torii_args.events.historical, vec!["a-A".to_string()]);
        assert_eq!(torii_args.server, ServerOptions::default());
        assert!(torii_args.indexing.transactions);
        assert_eq!(torii_args.database.page_size, 1024);
        assert_eq!(torii_args.database.cache_size, DEFAULT_DATABASE_CACHE_SIZE);
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
    fn test_config_fallback() {
        let content = r#"
        world_address = "0x1234"
        rpc = "http://0.0.0.0:2222"
        db_dir = "/tmp/torii-test"

        [events]
        raw = true
        historical = [
            "ns-E",
            "ns-EH"
        ]

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
        assert_eq!(torii_args.events.historical, vec!["ns-E".to_string(), "ns-EH".to_string()]);
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
