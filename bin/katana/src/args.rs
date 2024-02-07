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

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use common::parse::parse_socket_address;
use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::constants::{
    DEFAULT_GAS_PRICE, DEFAULT_INVOKE_MAX_STEPS, DEFAULT_VALIDATE_MAX_STEPS,
};
use katana_core::sequencer::SequencerConfig;
use katana_primitives::chain::ChainId;
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::constant::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use katana_primitives::genesis::Genesis;
use katana_rpc::config::ServerConfig;
use katana_rpc_api::ApiKind;
use tracing::Subscriber;
use tracing_subscriber::{fmt, EnvFilter};
use url::Url;

use crate::utils::{parse_genesis, parse_seed};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct KatanaArgs {
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

    #[arg(long)]
    #[arg(value_name = "URL")]
    #[arg(help = "The Starknet RPC provider to fork the network from.")]
    pub rpc_url: Option<Url>,

    #[arg(long)]
    pub dev: bool,

    #[arg(long)]
    #[arg(help = "Output logs in JSON format.")]
    pub json_log: bool,

    /// Enable Prometheus metrics.
    ///
    /// The metrics will be served at the given interface and port.
    #[arg(long, value_name = "SOCKET", value_parser = parse_socket_address, help_heading = "Metrics")]
    pub metrics: Option<SocketAddr>,

    #[arg(long)]
    #[arg(requires = "rpc_url")]
    #[arg(value_name = "BLOCK_NUMBER")]
    #[arg(help = "Fork the network at a specific block.")]
    pub fork_block_number: Option<u64>,

    #[cfg(feature = "messaging")]
    #[arg(long)]
    #[arg(value_name = "PATH")]
    #[arg(value_parser = katana_core::service::messaging::MessagingConfig::parse)]
    #[arg(help = "Configure the messaging with an other chain.")]
    #[arg(long_help = "Configure the messaging to allow Katana listening/sending messages on a \
                       settlement chain that can be Ethereum or an other Starknet sequencer. \
                       The configuration file details and examples can be found here: https://book.dojoengine.org/toolchain/katana/reference.html#messaging")]
    pub messaging: Option<katana_core::service::messaging::MessagingConfig>,

    #[command(flatten)]
    #[command(next_help_heading = "Server options")]
    pub server: ServerOptions,

    #[command(flatten)]
    #[command(next_help_heading = "Starknet options")]
    pub starknet: StarknetOptions,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(about = "Generate shell completion file for specified shell")]
    Completions { shell: Shell },
}

#[derive(Debug, Args, Clone)]
pub struct ServerOptions {
    #[arg(short, long)]
    #[arg(default_value = "5050")]
    #[arg(help = "Port number to listen on.")]
    pub port: u16,

    #[arg(long)]
    #[arg(help = "The IP address the server will listen on.")]
    pub host: Option<String>,

    #[arg(long)]
    #[arg(default_value = "100")]
    #[arg(help = "Maximum number of concurrent connections allowed.")]
    pub max_connections: u32,
}

#[derive(Debug, Args, Clone)]
pub struct StarknetOptions {
    #[arg(long)]
    #[arg(default_value = "0")]
    #[arg(help = "Specify the seed for randomness of accounts to be predeployed.")]
    pub seed: String,

    #[arg(long = "accounts")]
    #[arg(value_name = "NUM")]
    #[arg(default_value = "10")]
    #[arg(help = "Number of pre-funded accounts to generate.")]
    pub total_accounts: u16,

    #[arg(long)]
    #[arg(help = "Disable charging fee when executing transactions.")]
    pub disable_fee: bool,

    #[arg(long)]
    #[arg(help = "Disable validation when executing transactions.")]
    pub disable_validate: bool,

    #[command(flatten)]
    #[command(next_help_heading = "Environment options")]
    pub environment: EnvironmentOptions,

    #[arg(long)]
    #[arg(value_parser = parse_genesis)]
    #[arg(conflicts_with_all(["rpc_url", "seed", "total_accounts"]))]
    pub genesis: Option<Genesis>,
}

#[derive(Debug, Args, Clone)]
pub struct EnvironmentOptions {
    #[arg(long)]
    #[arg(help = "The chain ID.")]
    #[arg(long_help = "The chain ID. If a raw hex string (`0x` prefix) is provided, then it'd \
                       used as the actual chain ID. Otherwise, it's represented as the raw \
                       ASCII values. It must be a valid Cairo short string.")]
    #[arg(default_value = "KATANA")]
    #[arg(value_parser = ChainId::parse)]
    pub chain_id: ChainId,

    #[arg(long)]
    #[arg(help = "The gas price.")]
    pub gas_price: Option<u64>,

    #[arg(long)]
    #[arg(help = "The maximum number of steps available for the account validation logic.")]
    pub validate_max_steps: Option<u32>,

    #[arg(long)]
    #[arg(help = "The maximum number of steps available for the account execution logic.")]
    pub invoke_max_steps: Option<u32>,
}

impl KatanaArgs {
    pub fn init_logging(&self) -> Result<(), Box<dyn std::error::Error>> {
        const DEFAULT_LOG_FILTER: &str = "info,executor=trace,forked_backend=trace,server=debug,\
                                          katana_core=trace,blockifier=off,jsonrpsee_server=off,\
                                          hyper=off,messaging=debug,node=error";

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

    pub fn sequencer_config(&self) -> SequencerConfig {
        SequencerConfig {
            block_time: self.block_time,
            no_mining: self.no_mining,
            #[cfg(feature = "messaging")]
            messaging: self.messaging.clone(),
        }
    }

    pub fn server_config(&self) -> ServerConfig {
        let mut apis = vec![ApiKind::Starknet];
        // only enable `katana` API in dev mode
        if self.dev {
            apis.push(ApiKind::Katana);
        }

        ServerConfig {
            apis,
            port: self.server.port,
            host: self.server.host.clone().unwrap_or("0.0.0.0".into()),
            max_connections: self.server.max_connections,
        }
    }

    pub fn starknet_config(&self) -> StarknetConfig {
        let genesis = match self.starknet.genesis.clone() {
            Some(genesis) => genesis,
            None => {
                let accounts = DevAllocationsGenerator::new(self.starknet.total_accounts)
                    .with_seed(parse_seed(&self.starknet.seed))
                    .with_balance(DEFAULT_PREFUNDED_ACCOUNT_BALANCE)
                    .generate();

                let mut genesis = Genesis::default();
                genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
                genesis
            }
        };

        StarknetConfig {
            disable_fee: self.starknet.disable_fee,
            disable_validate: self.starknet.disable_validate,
            fork_rpc_url: self.rpc_url.clone(),
            fork_block_number: self.fork_block_number,
            env: Environment {
                chain_id: self.starknet.environment.chain_id,
                gas_price: self.starknet.environment.gas_price.unwrap_or(DEFAULT_GAS_PRICE),
                invoke_max_steps: self
                    .starknet
                    .environment
                    .invoke_max_steps
                    .unwrap_or(DEFAULT_INVOKE_MAX_STEPS),
                validate_max_steps: self
                    .starknet
                    .environment
                    .validate_max_steps
                    .unwrap_or(DEFAULT_VALIDATE_MAX_STEPS),
            },
            db_dir: self.db_dir.clone(),
            genesis,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn default_block_context_from_args() {
        let args = KatanaArgs::parse_from(["katana"]);
        let block_context = args.starknet_config().block_env();
        assert_eq!(block_context.l1_gas_prices.eth, DEFAULT_GAS_PRICE);
    }

    #[test]
    fn custom_block_context_from_args() {
        let args = KatanaArgs::parse_from([
            "katana",
            "--gas-price",
            "10",
            "--chain-id",
            "SN_GOERLI",
            "--validate-max-steps",
            "100",
            "--invoke-max-steps",
            "200",
        ]);

        let block_context = args.starknet_config().block_env();

        assert_eq!(block_context.l1_gas_prices.eth, 10);
    }
}
