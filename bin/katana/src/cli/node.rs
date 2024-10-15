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
use std::net::IpAddr;
use std::path::PathBuf;

use alloy_primitives::U256;
use anyhow::{Context, Result};
use clap::{Args, Parser};
use console::Style;
use katana_core::constants::DEFAULT_SEQUENCER_ADDRESS;
use katana_core::service::messaging::MessagingConfig;
use katana_node::config::db::DbConfig;
use katana_node::config::dev::{DevConfig, FixedL1GasPriceConfig};
use katana_node::config::execution::{
    DEFAULT_INVOCATION_MAX_STEPS, DEFAULT_VALIDATION_MAX_STEPS, ExecutionConfig,
};
use katana_node::config::fork::ForkingConfig;
use katana_node::config::metrics::{DEFAULT_METRICS_ADDR, DEFAULT_METRICS_PORT, MetricsConfig};
use katana_node::config::rpc::{
    ApiKind, DEFAULT_RPC_ADDR, DEFAULT_RPC_MAX_CONNECTIONS, DEFAULT_RPC_PORT, RpcConfig,
};
use katana_node::config::{Config, SequencingConfig};
use katana_primitives::block::{BlockHashOrNumber, GasPrices};
use katana_primitives::chain::ChainId;
use katana_primitives::chain_spec::{self, ChainSpec};
use katana_primitives::class::ClassHash;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::Genesis;
use katana_primitives::genesis::allocation::{DevAllocationsGenerator, GenesisAccountAlloc};
use katana_primitives::genesis::constant::{
    DEFAULT_LEGACY_ERC20_CLASS_HASH, DEFAULT_LEGACY_UDC_CLASS_HASH,
    DEFAULT_PREFUNDED_ACCOUNT_BALANCE, DEFAULT_UDC_ADDRESS,
};
use tracing::{Subscriber, info};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, fmt};
use url::Url;

use crate::utils::{parse_block_hash_or_number, parse_genesis, parse_seed};

#[derive(Parser, Debug)]
pub struct NodeArgs {
    #[arg(long)]
    #[arg(help = "Don't print anything on startup.")]
    pub silent: bool,

    #[arg(long)]
    #[arg(conflicts_with = "block_time")]
    #[arg(help = "Disable auto and interval mining, and mine on demand instead via an endpoint.")]
    pub no_mining: bool,

    #[arg(short, long)]
    #[arg(value_name = "MILLISECONDS")]
    #[arg(help = "Block time in milliseconds for interval mining.")]
    pub block_time: Option<u64>,

    #[arg(long)]
    #[arg(value_name = "PATH")]
    #[arg(help = "Directory path of the database to initialize from.")]
    #[arg(long_help = "Directory path of the database to initialize from. The path must either \
                       be an empty directory or a directory which already contains a previously \
                       initialized Katana database.")]
    pub db_dir: Option<PathBuf>,

    #[arg(long = "fork.rpc-url", value_name = "URL", alias = "rpc-url")]
    #[arg(help = "The Starknet RPC provider to fork the network from.")]
    pub fork_rpc_url: Option<Url>,

    #[arg(long = "fork.block", value_name = "BLOCK_ID", alias = "fork-block-number")]
    #[arg(requires = "fork_rpc_url")]
    #[arg(help = "Fork the network at a specific block id, can either be a hash (0x-prefixed) \
                  or number.")]
    #[arg(value_parser = parse_block_hash_or_number)]
    pub fork_block: Option<BlockHashOrNumber>,

    #[arg(long)]
    #[arg(help = "Output logs in JSON format.")]
    pub json_log: bool,

    #[arg(long)]
    #[arg(value_name = "PATH")]
    #[arg(value_parser = katana_core::service::messaging::MessagingConfig::parse)]
    #[arg(help = "Configure the messaging with an other chain.")]
    #[arg(long_help = "Configure the messaging to allow Katana listening/sending messages on a \
                       settlement chain that can be Ethereum or an other Starknet sequencer. \
                       The configuration file details and examples can be found here: https://book.dojoengine.org/toolchain/katana/reference#messaging")]
    pub messaging: Option<MessagingConfig>,

    #[command(flatten)]
    pub metrics: MetricsOptions,

    #[command(flatten)]
    pub server: ServerOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub development: DevOptions,

    #[cfg(feature = "slot")]
    #[command(flatten)]
    pub slot: SlotOptions,
}

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Metrics options")]
pub struct MetricsOptions {
    /// Whether to enable metrics.
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

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Server options")]
pub struct ServerOptions {
    #[arg(long = "http.addr", value_name = "ADDRESS")]
    #[arg(default_value_t = DEFAULT_RPC_ADDR)]
    #[arg(help = "The IP address the server will listen on.")]
    pub http_addr: IpAddr,

    #[arg(long = "http.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_RPC_PORT)]
    #[arg(help = "Port number to listen on.")]
    pub http_port: u16,

    #[arg(long = "http.corsdomain")]
    #[arg(value_delimiter = ',')]
    #[arg(help = "Enables the CORS layer and sets the allowed origins, separated by commas.")]
    pub http_cors_domain: Option<Vec<String>>,

    #[arg(long = "rpc.max-connections", value_name = "COUNT")]
    #[arg(default_value_t = DEFAULT_RPC_MAX_CONNECTIONS)]
    #[arg(help = "Maximum number of concurrent connections allowed.")]
    pub max_connections: u32,
}

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Starknet options")]
pub struct StarknetOptions {
    #[command(flatten)]
    pub environment: EnvironmentOptions,

    #[arg(long)]
    #[arg(value_parser = parse_genesis)]
    #[arg(conflicts_with_all(["fork_rpc_url", "seed", "total_accounts"]))]
    pub genesis: Option<Genesis>,
}

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Environment options")]
pub struct EnvironmentOptions {
    #[arg(long)]
    #[arg(help = "The chain ID.")]
    #[arg(long_help = "The chain ID. If a raw hex string (`0x` prefix) is provided, then it'd \
                       used as the actual chain ID. Otherwise, it's represented as the raw \
                       ASCII values. It must be a valid Cairo short string.")]
    #[arg(value_parser = ChainId::parse)]
    pub chain_id: Option<ChainId>,

    #[arg(long)]
    #[arg(help = "The maximum number of steps available for the account validation logic.")]
    pub validate_max_steps: Option<u32>,

    #[arg(long)]
    #[arg(help = "The maximum number of steps available for the account execution logic.")]
    pub invoke_max_steps: Option<u32>,

    #[arg(long = "l1-eth-gas-price", value_name = "WEI")]
    #[arg(help = "The L1 ETH gas price. (denominated in wei)")]
    #[arg(requires = "l1_strk_gas_price")]
    pub l1_eth_gas_price: Option<u128>,

    #[arg(long = "l1-strk-gas-price", value_name = "FRI")]
    #[arg(requires = "l1_eth_data_gas_price")]
    #[arg(help = "The L1 STRK gas price. (denominated in fri)")]
    pub l1_strk_gas_price: Option<u128>,

    #[arg(long = "l1-eth-data-gas-price", value_name = "WEI")]
    #[arg(requires = "l1_strk_data_gas_price")]
    #[arg(help = "The L1 ETH gas price. (denominated in wei)")]
    pub l1_eth_data_gas_price: Option<u128>,

    #[arg(long = "l1-strk-data-gas-price", value_name = "FRI")]
    #[arg(requires = "l1_eth_gas_price")]
    #[arg(help = "The L1 STRK gas prick. (denominated in fri)")]
    pub l1_strk_data_gas_price: Option<u128>,
}

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Development options")]
pub struct DevOptions {
    /// Enable development mode.
    #[arg(long)]
    pub dev: bool,

    /// Specify the seed for randomness of accounts to be predeployed.
    #[arg(requires = "dev")]
    #[arg(long = "dev.seed", default_value = "0")]
    pub seed: String,

    /// Number of pre-funded accounts to generate.
    #[arg(requires = "dev")]
    #[arg(long = "dev.accounts", value_name = "NUM")]
    #[arg(default_value_t = 10)]
    pub total_accounts: u16,

    /// Disable charging fee when executing transactions.
    #[arg(requires = "dev")]
    #[arg(long = "dev.disable-fee")]
    pub disable_fee: bool,

    /// Skip account validation when executing transactions.
    #[arg(requires = "dev")]
    #[arg(long = "dev.disable-account-validation")]
    pub disable_validate: bool,
}

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Development options")]
pub struct DevOptions {
    /// Enable development mode.
    #[arg(long)]
    pub dev: bool,

    /// Specify the seed for randomness of accounts to be predeployed.
    #[arg(requires = "dev")]
    #[arg(long = "dev.seed", default_value = "0")]
    pub seed: String,

    /// Number of pre-funded accounts to generate.
    #[arg(requires = "dev")]
    #[arg(long = "dev.accounts", value_name = "NUM")]
    #[arg(default_value_t = 10)]
    pub total_accounts: u16,

    /// Disable charging fee when executing transactions.
    #[arg(requires = "dev")]
    #[arg(long = "dev.disable-fee")]
    pub disable_fee: bool,

    /// Skip account validation when executing transactions.
    #[arg(requires = "dev")]
    #[arg(long = "dev.disable-account-validation")]
    pub disable_validate: bool,
}

#[cfg(feature = "slot")]
#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Slot options")]
pub struct SlotOptions {
    #[arg(hide = true)]
    #[arg(long = "slot.controller")]
    pub controller: bool,
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
                                          server=debug,blockifier=off,jsonrpsee_server=off,\
                                          hyper=off,messaging=debug,node=error";

        LogTracer::init()?;

        let builder = fmt::Subscriber::builder().with_env_filter(
            EnvFilter::try_from_default_env().or(EnvFilter::try_new(DEFAULT_LOG_FILTER))?,
        );

        let subscriber: Box<dyn Subscriber + Send + Sync> = if self.json_log {
            Box::new(builder.json().finish())
        } else {
            Box::new(builder.finish())
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
            cors_domain: self.server.http_cors_domain.clone(),
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
        let fixed_gas_prices = if self.starknet.environment.l1_eth_gas_price.is_some() {
            // It is safe to unwrap all of these here because the CLI parser ensures if one is set,
            // all must be set.

            let eth_gas_price = self.starknet.environment.l1_eth_gas_price.unwrap();
            let strk_gas_price = self.starknet.environment.l1_strk_gas_price.unwrap();
            let eth_data_gas_price = self.starknet.environment.l1_eth_data_gas_price.unwrap();
            let strk_data_gas_price = self.starknet.environment.l1_strk_data_gas_price.unwrap();

            let gas_price = GasPrices { eth: eth_gas_price, strk: strk_gas_price };
            let data_gas_price = GasPrices { eth: eth_data_gas_price, strk: strk_data_gas_price };

            Some(FixedL1GasPriceConfig { gas_price, data_gas_price })
        } else {
            None
        };

        DevConfig {
            fixed_gas_prices,
            fee: !self.development.disable_fee,
            account_validation: !self.development.disable_validate,
        }
    }

    fn execution_config(&self) -> ExecutionConfig {
        ExecutionConfig {
            invocation_max_steps: self
                .starknet
                .environment
                .invoke_max_steps
                .unwrap_or(DEFAULT_INVOCATION_MAX_STEPS),
            validation_max_steps: self
                .starknet
                .environment
                .validate_max_steps
                .unwrap_or(DEFAULT_VALIDATION_MAX_STEPS),
            ..Default::default()
        }
    }

    fn forking_config(&self) -> Result<Option<ForkingConfig>> {
        if let Some(url) = self.fork_rpc_url.clone() {
            Ok(Some(ForkingConfig { url, block: self.fork_block }))
        } else {
            Ok(None)
        }
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
}

fn print_intro(args: &NodeArgs, chain: &ChainSpec) {
    let mut accounts = chain.genesis.accounts().peekable();
    let account_class_hash = accounts.peek().map(|e| e.1.class_hash());
    let seed = &args.development.seed;

    if args.json_log {
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
    use assert_matches::assert_matches;
    use katana_primitives::{address, felt};

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
            "--disable-fee",
            "--disable-validate",
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
        let args = NodeArgs::parse_from([
            "katana",
            "--disable-fee",
            "--disable-validate",
            "--chain-id",
            "SN_GOERLI",
            "--invoke-max-steps",
            "200",
            "--validate-max-steps",
            "100",
            "--db-dir",
            "/path/to/db",
            "--l1-eth-gas-price",
            "10",
            "--l1-strk-gas-price",
            "20",
            "--l1-eth-data-gas-price",
            "1",
            "--l1-strk-data-gas-price",
            "2",
        ]);
        let config = args.config().unwrap();

        assert!(!config.dev.fee);
        assert!(!config.dev.account_validation);
        assert_eq!(config.execution.invocation_max_steps, 200);
        assert_eq!(config.execution.validation_max_steps, 100);
        assert_eq!(config.db.dir, Some(PathBuf::from("/path/to/db")));
        assert_eq!(config.chain.id, ChainId::GOERLI);
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
            "--l1-eth-gas-price",
            "100",
            "--l1-strk-gas-price",
            "200",
            "--l1-eth-data-gas-price",
            "111",
            "--l1-strk-data-gas-price",
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
}
