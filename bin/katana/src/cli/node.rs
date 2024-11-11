//! Katana binary executable.
//!
//! ## Feature Flags
//!
//! - `jemalloc`: Uses [jemallocator](https://github.com/tikv/jemallocator) as the global allocator.
//!   This is **not recommended on Windows**. See [here](https://rust-lang.github.io/rfcs/1974-global-allocators.html#jemalloc)
//!   for more info.
//! - `jemalloc-prof`: Enables [jemallocator's](https://github.com/tikv/jemallocator) heap profiling
//!   and leak detection functionality. See [jemalloc's opt.prof](https://jemalloc.net/jemalloc.3.html#opt.prof)
//!   documentation for usage details. This is **not recommended on Windows**. See [here](https://rust-lang.github.io/rfcs/1974-global-allocators.html#jemalloc)
//!   for more info.

use std::collections::HashSet;
use std::path::PathBuf;

use alloy_primitives::U256;
use anyhow::{Context, Result};
use clap::Parser;
use console::Style;
use katana_core::constants::DEFAULT_SEQUENCER_ADDRESS;
use katana_core::service::messaging::MessagingConfig;
use katana_node::config::db::DbConfig;
use katana_node::config::dev::{DevConfig, FixedL1GasPriceConfig};
use katana_node::config::execution::ExecutionConfig;
use katana_node::config::fork::ForkingConfig;
use katana_node::config::metrics::MetricsConfig;
use katana_node::config::rpc::{ApiKind, RpcConfig};
use katana_node::config::{Config, SequencingConfig};
use katana_primitives::chain_spec::{self, ChainSpec};
use katana_primitives::class::ClassHash;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::{DevAllocationsGenerator, GenesisAccountAlloc};
use katana_primitives::genesis::constant::{
    DEFAULT_LEGACY_ERC20_CLASS_HASH, DEFAULT_LEGACY_UDC_CLASS_HASH,
    DEFAULT_PREFUNDED_ACCOUNT_BALANCE, DEFAULT_UDC_ADDRESS,
};
use serde::{Deserialize, Serialize};
use tracing::{info, Subscriber};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, EnvFilter};

use super::options::*;
use crate::utils::{parse_seed, LogFormat};

#[derive(Parser, Debug, Serialize, Deserialize, Default)]
pub struct NodeArgs {
    /// Don't print anything on startup.
    #[arg(long)]
    pub silent: bool,

    /// Disable auto and interval mining, and mine on demand instead via an endpoint.
    #[arg(long)]
    #[arg(conflicts_with = "block_time")]
    pub no_mining: bool,

    /// Block time in milliseconds for interval mining.
    #[arg(short, long)]
    #[arg(value_name = "MILLISECONDS")]
    pub block_time: Option<u64>,

    /// Directory path of the database to initialize from.
    ///
    /// The path must either be an empty directory or a directory which already contains a
    /// previously initialized Katana database.
    #[arg(long)]
    #[arg(value_name = "PATH")]
    pub db_dir: Option<PathBuf>,

    /// Configure the messaging with an other chain.
    ///
    /// Configure the messaging to allow Katana listening/sending messages on a
    /// settlement chain that can be Ethereum or an other Starknet sequencer.
    #[arg(long)]
    #[arg(value_name = "PATH")]
    #[arg(value_parser = katana_core::service::messaging::MessagingConfig::parse)]
    pub messaging: Option<MessagingConfig>,

    #[command(flatten)]
    pub logging: LoggingOptions,

    #[command(flatten)]
    pub metrics: MetricsOptions,

    #[command(flatten)]
    pub server: ServerOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub gpo: GasPriceOracleOptions,

    #[command(flatten)]
    pub forking: ForkingOptions,

    #[command(flatten)]
    pub development: DevOptions,

    #[cfg(feature = "slot")]
    #[command(flatten)]
    pub slot: SlotOptions,

    /// Configuration file
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NodeArgsConfig {
    pub silent: Option<bool>,
    pub no_mining: Option<bool>,
    pub block_time: Option<u64>,
    pub db_dir: Option<PathBuf>,
    pub messaging: Option<MessagingConfig>,
    pub logging: Option<LoggingOptions>,
    pub metrics: Option<MetricsOptions>,
    pub server: Option<ServerOptions>,
    pub starknet: Option<StarknetOptions>,
    pub gpo: Option<GasPriceOracleOptions>,
    pub forking: Option<ForkingOptions>,
    #[serde(rename = "dev")]
    pub development: Option<DevOptions>,

    #[cfg(feature = "slot")]
    pub slot: Option<SlotOptions>,
}

pub(crate) const LOG_TARGET: &str = "katana::cli";

impl NodeArgs {
    pub fn execute(self) -> Result<()> {
        self.init_logging()?;
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime")?
            .block_on(self.start_node())
    }

    async fn start_node(self) -> Result<()> {
        // Build the node
        let config = self.config()?;
        let node = katana_node::build(config).await.context("failed to build node")?;

        if !self.silent {
            print_intro(&self, &node.backend.chain_spec);
        }

        // Launch the node
        let handle = node.launch().await.context("failed to launch node")?;

        // Wait until an OS signal (ie SIGINT, SIGTERM) is received or the node is shutdown.
        tokio::select! {
            _ = dojo_utils::signal::wait_signals() => {
                // Gracefully shutdown the node before exiting
                handle.stop().await?;
            },

            _ = handle.stopped() => { }
        }

        info!("Shutting down.");

        Ok(())
    }

    fn init_logging(&self) -> Result<()> {
        const DEFAULT_LOG_FILTER: &str = "info,tasks=debug,executor=trace,forking::backend=trace,\
                                          blockifier=off,jsonrpsee_server=off,hyper=off,\
                                          messaging=debug,node=error";

        let filter = if self.development.dev {
            &format!("{DEFAULT_LOG_FILTER},server=debug")
        } else {
            DEFAULT_LOG_FILTER
        };

        LogTracer::init()?;

        // If the user has set the `RUST_LOG` environment variable, then we prioritize it.
        // Otherwise, we use the default log filter.
        // TODO: change env var to `KATANA_LOG`.
        let filter = EnvFilter::try_from_default_env().or(EnvFilter::try_new(filter))?;
        let builder = fmt::Subscriber::builder().with_env_filter(filter);

        let subscriber: Box<dyn Subscriber + Send + Sync> = match self.logging.log_format {
            LogFormat::Full => Box::new(builder.finish()),
            LogFormat::Json => Box::new(builder.json().finish()),
        };

        Ok(tracing::subscriber::set_global_default(subscriber)?)
    }

    fn config(&self) -> Result<katana_node::config::Config> {
        let db = self.db_config();
        let rpc = self.rpc_config();
        let dev = self.dev_config();
        let chain = self.chain_spec()?;
        let metrics = self.metrics_config();
        let forking = self.forking_config()?;
        let execution = self.execution_config();
        let sequencing = self.sequencer_config();
        let messaging = self.messaging.clone();

        Ok(Config { metrics, db, dev, rpc, chain, execution, sequencing, messaging, forking })
    }

    fn sequencer_config(&self) -> SequencingConfig {
        SequencingConfig { block_time: self.block_time, no_mining: self.no_mining }
    }

    fn rpc_config(&self) -> RpcConfig {
        let mut apis = HashSet::from([ApiKind::Starknet, ApiKind::Torii, ApiKind::Saya]);
        // only enable `katana` API in dev mode
        if self.development.dev {
            apis.insert(ApiKind::Dev);
        }

        RpcConfig {
            apis,
            port: self.server.http_port,
            addr: self.server.http_addr,
            max_connections: self.server.max_connections,
            cors_origins: self.server.http_cors_origins.clone(),
        }
    }

    fn chain_spec(&self) -> Result<ChainSpec> {
        let mut chain_spec = chain_spec::DEV_UNALLOCATED.clone();

        if let Some(id) = self.starknet.environment.chain_id {
            chain_spec.id = id;
        }

        if let Some(genesis) = self.starknet.genesis.clone() {
            chain_spec.genesis = genesis;
        } else {
            chain_spec.genesis.sequencer_address = *DEFAULT_SEQUENCER_ADDRESS;
        }

        // generate dev accounts
        let accounts = DevAllocationsGenerator::new(self.development.total_accounts)
            .with_seed(parse_seed(&self.development.seed))
            .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
            .generate();

        chain_spec.genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));

        #[cfg(feature = "slot")]
        if self.slot.controller {
            katana_slot_controller::add_controller_account(&mut chain_spec.genesis)?;
        }

        Ok(chain_spec)
    }

    fn dev_config(&self) -> DevConfig {
        let mut fixed_gas_prices = None;

        if self.gpo.l1_eth_gas_price > 0 {
            let prices = fixed_gas_prices.get_or_insert(FixedL1GasPriceConfig::default());
            prices.gas_price.eth = self.gpo.l1_eth_gas_price;
        }

        if self.gpo.l1_strk_gas_price > 0 {
            let prices = fixed_gas_prices.get_or_insert(FixedL1GasPriceConfig::default());
            prices.gas_price.strk = self.gpo.l1_strk_gas_price;
        }

        if self.gpo.l1_eth_data_gas_price > 0 {
            let prices = fixed_gas_prices.get_or_insert(FixedL1GasPriceConfig::default());
            prices.data_gas_price.eth = self.gpo.l1_eth_data_gas_price;
        }

        if self.gpo.l1_strk_data_gas_price > 0 {
            let prices = fixed_gas_prices.get_or_insert(FixedL1GasPriceConfig::default());
            prices.data_gas_price.strk = self.gpo.l1_strk_data_gas_price;
        }

        DevConfig {
            fixed_gas_prices,
            fee: !self.development.no_fee,
            account_validation: !self.development.no_account_validation,
        }
    }

    fn execution_config(&self) -> ExecutionConfig {
        ExecutionConfig {
            invocation_max_steps: self.starknet.environment.invoke_max_steps,
            validation_max_steps: self.starknet.environment.validate_max_steps,
            ..Default::default()
        }
    }

    fn forking_config(&self) -> Result<Option<ForkingConfig>> {
        if let Some(ref url) = self.forking.fork_provider {
            let cfg = ForkingConfig { url: url.clone(), block: self.forking.fork_block };
            return Ok(Some(cfg));
        }

        Ok(None)
    }

    fn db_config(&self) -> DbConfig {
        DbConfig { dir: self.db_dir.clone() }
    }

    fn metrics_config(&self) -> Option<MetricsConfig> {
        if self.metrics.metrics {
            Some(MetricsConfig { addr: self.metrics.metrics_addr, port: self.metrics.metrics_port })
        } else {
            None
        }
    }

    /// Parse the node config from the command line arguments and the config file,
    /// and merge them together prioritizing the command line arguments.
    pub fn with_config_file(mut self) -> Result<Self> {
        let config: NodeArgsConfig = if let Some(path) = &self.config {
            toml::from_str(&std::fs::read_to_string(path)?)?
        } else {
            return Ok(self);
        };

        // the CLI (self) takes precedence over the config file.
        // Currently, the merge is made at the top level of the commands.
        // We may add recursive merging in the future.

        if !self.silent {
            self.silent = config.silent.unwrap_or_default();
        }

        if !self.no_mining {
            self.no_mining = config.no_mining.unwrap_or_default();
        }

        if self.block_time.is_none() {
            self.block_time = config.block_time;
        }

        if self.db_dir.is_none() {
            self.db_dir = config.db_dir;
        }

        if self.logging == LoggingOptions::default() {
            if let Some(logging) = config.logging {
                self.logging = logging;
            }
        }

        if self.metrics == MetricsOptions::default() {
            if let Some(metrics) = config.metrics {
                self.metrics = metrics;
            }
        }

        if self.server == ServerOptions::default() {
            if let Some(server) = config.server {
                self.server = server;
            }
        }

        self.starknet.merge(config.starknet.as_ref());

        if self.gpo == GasPriceOracleOptions::default() {
            if let Some(gpo) = config.gpo {
                self.gpo = gpo;
            }
        }

        if self.forking == ForkingOptions::default() {
            if let Some(forking) = config.forking {
                self.forking = forking;
            }
        }

        if self.development == DevOptions::default() {
            if let Some(development) = config.development {
                self.development = development;
            }
        }

        #[cfg(feature = "slot")]
        if self.slot == SlotOptions::default() {
            if let Some(slot) = config.slot {
                self.slot = slot;
            }
        }

        Ok(self)
    }
}

fn print_intro(args: &NodeArgs, chain: &ChainSpec) {
    let mut accounts = chain.genesis.accounts().peekable();
    let account_class_hash = accounts.peek().map(|e| e.1.class_hash());
    let seed = &args.development.seed;

    if args.logging.log_format == LogFormat::Json {
        info!(
            target: LOG_TARGET,
            "{}",
            serde_json::json!({
                "accounts": accounts.map(|a| serde_json::json!(a)).collect::<Vec<_>>(),
                "seed": format!("{}", seed),
            })
        )
    } else {
        println!(
            "{}",
            Style::new().red().apply_to(
                r"


██╗  ██╗ █████╗ ████████╗ █████╗ ███╗   ██╗ █████╗
██║ ██╔╝██╔══██╗╚══██╔══╝██╔══██╗████╗  ██║██╔══██╗
█████╔╝ ███████║   ██║   ███████║██╔██╗ ██║███████║
██╔═██╗ ██╔══██║   ██║   ██╔══██║██║╚██╗██║██╔══██║
██║  ██╗██║  ██║   ██║   ██║  ██║██║ ╚████║██║  ██║
╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝
"
            )
        );

        print_genesis_contracts(chain, account_class_hash);
        print_genesis_accounts(accounts);

        println!(
            r"

ACCOUNTS SEED
=============
{seed}
    "
        );
    }
}

fn print_genesis_contracts(chain: &ChainSpec, account_class_hash: Option<ClassHash>) {
    println!(
        r"
PREDEPLOYED CONTRACTS
==================

| Contract        | ETH Fee Token
| Address         | {}
| Class Hash      | {:#064x}

| Contract        | STRK Fee Token
| Address         | {}
| Class Hash      | {:#064x}",
        chain.fee_contracts.eth,
        DEFAULT_LEGACY_ERC20_CLASS_HASH,
        chain.fee_contracts.strk,
        DEFAULT_LEGACY_ERC20_CLASS_HASH
    );

    println!(
        r"
| Contract        | Universal Deployer
| Address         | {}
| Class Hash      | {:#064x}",
        DEFAULT_UDC_ADDRESS, DEFAULT_LEGACY_UDC_CLASS_HASH
    );

    if let Some(hash) = account_class_hash {
        println!(
            r"
| Contract        | Account Contract
| Class Hash      | {hash:#064x}"
        )
    }
}

fn print_genesis_accounts<'a, Accounts>(accounts: Accounts)
where
    Accounts: Iterator<Item = (&'a ContractAddress, &'a GenesisAccountAlloc)>,
{
    println!(
        r"

PREFUNDED ACCOUNTS
=================="
    );

    for (addr, account) in accounts {
        if let Some(pk) = account.private_key() {
            println!(
                r"
| Account address |  {addr}
| Private key     |  {pk:#x}
| Public key      |  {:#x}",
                account.public_key()
            )
        } else {
            println!(
                r"
| Account address |  {addr}
| Public key      |  {:#x}",
                account.public_key()
            )
        }
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use assert_matches::assert_matches;
    use katana_core::constants::{
        DEFAULT_ETH_L1_DATA_GAS_PRICE, DEFAULT_ETH_L1_GAS_PRICE, DEFAULT_STRK_L1_DATA_GAS_PRICE,
        DEFAULT_STRK_L1_GAS_PRICE,
    };
    use katana_node::config::execution::{
        DEFAULT_INVOCATION_MAX_STEPS, DEFAULT_VALIDATION_MAX_STEPS,
    };
    use katana_primitives::chain::ChainId;
    use katana_primitives::{address, felt, Felt};

    use super::*;

    #[test]
    fn test_starknet_config_default() {
        let args = NodeArgs::parse_from(["katana"]);
        let config = args.config().unwrap();

        assert!(config.dev.fee);
        assert!(config.dev.account_validation);
        assert!(config.forking.is_none());
        assert_eq!(config.execution.invocation_max_steps, DEFAULT_INVOCATION_MAX_STEPS);
        assert_eq!(config.execution.validation_max_steps, DEFAULT_VALIDATION_MAX_STEPS);
        assert_eq!(config.db.dir, None);
        assert_eq!(config.chain.id, ChainId::parse("KATANA").unwrap());
        assert_eq!(config.chain.genesis.sequencer_address, *DEFAULT_SEQUENCER_ADDRESS);
    }

    #[test]
    fn test_starknet_config_custom() {
        let args = NodeArgs::parse_from([
            "katana",
            "--dev",
            "--dev.no-fee",
            "--dev.no-account-validation",
            "--chain-id",
            "SN_GOERLI",
            "--invoke-max-steps",
            "200",
            "--validate-max-steps",
            "100",
            "--db-dir",
            "/path/to/db",
        ]);
        let config = args.config().unwrap();

        assert!(!config.dev.fee);
        assert!(!config.dev.account_validation);
        assert_eq!(config.execution.invocation_max_steps, 200);
        assert_eq!(config.execution.validation_max_steps, 100);
        assert_eq!(config.db.dir, Some(PathBuf::from("/path/to/db")));
        assert_eq!(config.chain.id, ChainId::GOERLI);
        assert_eq!(config.chain.genesis.sequencer_address, *DEFAULT_SEQUENCER_ADDRESS);
    }

    #[test]
    fn custom_fixed_gas_prices() {
        let config = NodeArgs::parse_from(["katana"]).config().unwrap();
        assert!(config.dev.fixed_gas_prices.is_none());

        let config =
            NodeArgs::parse_from(["katana", "--gpo.l1-eth-gas-price", "10"]).config().unwrap();
        assert_matches!(config.dev.fixed_gas_prices, Some(prices) => {
            assert_eq!(prices.gas_price.eth, 10);
            assert_eq!(prices.gas_price.strk, DEFAULT_STRK_L1_GAS_PRICE);
            assert_eq!(prices.data_gas_price.eth, DEFAULT_ETH_L1_DATA_GAS_PRICE);
            assert_eq!(prices.data_gas_price.strk, DEFAULT_STRK_L1_DATA_GAS_PRICE);
        });

        let config =
            NodeArgs::parse_from(["katana", "--gpo.l1-strk-gas-price", "20"]).config().unwrap();
        assert_matches!(config.dev.fixed_gas_prices, Some(prices) => {
            assert_eq!(prices.gas_price.eth, DEFAULT_ETH_L1_GAS_PRICE);
            assert_eq!(prices.gas_price.strk, 20);
            assert_eq!(prices.data_gas_price.eth, DEFAULT_ETH_L1_DATA_GAS_PRICE);
            assert_eq!(prices.data_gas_price.strk, DEFAULT_STRK_L1_DATA_GAS_PRICE);
        });

        let config =
            NodeArgs::parse_from(["katana", "--gpo.l1-eth-data-gas-price", "1"]).config().unwrap();
        assert_matches!(config.dev.fixed_gas_prices, Some(prices) => {
            assert_eq!(prices.gas_price.eth, DEFAULT_ETH_L1_GAS_PRICE);
            assert_eq!(prices.gas_price.strk, DEFAULT_STRK_L1_GAS_PRICE);
            assert_eq!(prices.data_gas_price.eth, 1);
            assert_eq!(prices.data_gas_price.strk, DEFAULT_STRK_L1_DATA_GAS_PRICE);
        });

        let config =
            NodeArgs::parse_from(["katana", "--gpo.l1-strk-data-gas-price", "2"]).config().unwrap();
        assert_matches!(config.dev.fixed_gas_prices, Some(prices) => {
            assert_eq!(prices.gas_price.eth, DEFAULT_ETH_L1_GAS_PRICE);
            assert_eq!(prices.gas_price.strk, DEFAULT_STRK_L1_GAS_PRICE);
            assert_eq!(prices.data_gas_price.eth, DEFAULT_ETH_L1_DATA_GAS_PRICE);
            assert_eq!(prices.data_gas_price.strk, 2);
        });

        let config = NodeArgs::parse_from([
            "katana",
            "--gpo.l1-eth-gas-price",
            "10",
            "--gpo.l1-strk-data-gas-price",
            "2",
        ])
        .config()
        .unwrap();

        assert_matches!(config.dev.fixed_gas_prices, Some(prices) => {
            assert_eq!(prices.gas_price.eth, 10);
            assert_eq!(prices.gas_price.strk, DEFAULT_STRK_L1_GAS_PRICE);
            assert_eq!(prices.data_gas_price.eth, DEFAULT_ETH_L1_DATA_GAS_PRICE);
            assert_eq!(prices.data_gas_price.strk, 2);
        });

        // Set all the gas prices options

        let config = NodeArgs::parse_from([
            "katana",
            "--gpo.l1-eth-gas-price",
            "10",
            "--gpo.l1-strk-gas-price",
            "20",
            "--gpo.l1-eth-data-gas-price",
            "1",
            "--gpo.l1-strk-data-gas-price",
            "2",
        ])
        .config()
        .unwrap();

        assert_matches!(config.dev.fixed_gas_prices, Some(prices) => {
            assert_eq!(prices.gas_price.eth, 10);
            assert_eq!(prices.gas_price.strk, 20);
            assert_eq!(prices.data_gas_price.eth, 1);
            assert_eq!(prices.data_gas_price.strk, 2);
        })
    }

    #[test]
    fn genesis_with_fixed_gas_prices() {
        let config = NodeArgs::parse_from([
            "katana",
            "--genesis",
            "./tests/test-data/genesis.json",
            "--gpo.l1-eth-gas-price",
            "100",
            "--gpo.l1-strk-gas-price",
            "200",
            "--gpo.l1-eth-data-gas-price",
            "111",
            "--gpo.l1-strk-data-gas-price",
            "222",
        ])
        .config()
        .unwrap();

        assert_eq!(config.chain.genesis.number, 0);
        assert_eq!(config.chain.genesis.parent_hash, felt!("0x999"));
        assert_eq!(config.chain.genesis.timestamp, 5123512314);
        assert_eq!(config.chain.genesis.state_root, felt!("0x99"));
        assert_eq!(config.chain.genesis.sequencer_address, address!("0x100"));
        assert_eq!(config.chain.genesis.gas_prices.eth, 9999);
        assert_eq!(config.chain.genesis.gas_prices.strk, 8888);
        assert_matches!(config.dev.fixed_gas_prices, Some(prices) => {
            assert_eq!(prices.gas_price.eth, 100);
            assert_eq!(prices.gas_price.strk, 200);
            assert_eq!(prices.data_gas_price.eth, 111);
            assert_eq!(prices.data_gas_price.strk, 222);
        })
    }

    #[test]
    fn config_from_file_and_cli() {
        // CLI args must take precedence over the config file.
        let content = r#"
[gpo]
l1_eth_gas_price = "0xfe"
l1_strk_gas_price = "200"
l1_eth_data_gas_price = "111"
l1_strk_data_gas_price = "222"

[dev]
total_accounts = 20

[starknet.env]
validate_max_steps = 500
chain_id.Named = "Mainnet"
        "#;
        let path = std::env::temp_dir().join("katana-config.json");
        std::fs::write(&path, content).unwrap();

        let path_str = path.to_string_lossy().to_string();

        let args = vec![
            "katana",
            "--config",
            path_str.as_str(),
            "--genesis",
            "./tests/test-data/genesis.json",
            "--validate-max-steps",
            "1234",
            "--dev",
            "--dev.no-fee",
            "--chain-id",
            "0x123",
        ];

        let config =
            NodeArgs::parse_from(args.clone()).with_config_file().unwrap().config().unwrap();

        assert_eq!(config.execution.validation_max_steps, 1234);
        assert_eq!(config.execution.invocation_max_steps, 10_000_000);
        assert!(!config.dev.fee);
        assert_matches!(config.dev.fixed_gas_prices, Some(prices) => {
            assert_eq!(prices.gas_price.eth, 254);
            assert_eq!(prices.gas_price.strk, 200);
            assert_eq!(prices.data_gas_price.eth, 111);
            assert_eq!(prices.data_gas_price.strk, 222);
        });
        assert_eq!(config.chain.genesis.number, 0);
        assert_eq!(config.chain.genesis.parent_hash, felt!("0x999"));
        assert_eq!(config.chain.genesis.timestamp, 5123512314);
        assert_eq!(config.chain.genesis.state_root, felt!("0x99"));
        assert_eq!(config.chain.genesis.sequencer_address, address!("0x100"));
        assert_eq!(config.chain.genesis.gas_prices.eth, 9999);
        assert_eq!(config.chain.genesis.gas_prices.strk, 8888);
        assert_eq!(config.chain.id, ChainId::Id(Felt::from_str("0x123").unwrap()));
    }
}
