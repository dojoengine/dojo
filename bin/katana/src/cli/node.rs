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

use alloy_primitives::U256;
use anyhow::{Context, Result};
use clap::{Args, Parser};
use console::Style;
use dojo_utils::parse::parse_socket_address;
use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::constants::{
    DEFAULT_ETH_L1_GAS_PRICE, DEFAULT_INVOKE_MAX_STEPS, DEFAULT_SEQUENCER_ADDRESS,
    DEFAULT_STRK_L1_GAS_PRICE, DEFAULT_VALIDATE_MAX_STEPS,
};
#[allow(deprecated)]
use katana_core::sequencer::SequencerConfig;
use katana_primitives::block::GasPrices;
use katana_primitives::chain::ChainId;
use katana_primitives::class::ClassHash;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::allocation::{DevAllocationsGenerator, GenesisAccountAlloc};
use katana_primitives::genesis::constant::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use katana_primitives::genesis::Genesis;
use katana_rpc::config::ServerConfig;
use katana_rpc_api::ApiKind;
use tokio::signal::ctrl_c;
use tracing::{info, Subscriber};
use tracing_subscriber::{fmt, EnvFilter};
use url::Url;

use crate::utils::{parse_genesis, parse_seed};

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
                       The configuration file details and examples can be found here: https://book.dojoengine.org/toolchain/katana/reference#messaging")]
    pub messaging: Option<katana_core::service::messaging::MessagingConfig>,

    #[command(flatten)]
    #[command(next_help_heading = "Server options")]
    pub server: ServerOptions,

    #[command(flatten)]
    #[command(next_help_heading = "Starknet options")]
    pub starknet: StarknetOptions,

    #[cfg(feature = "slot")]
    #[command(flatten)]
    #[command(next_help_heading = "Slot options")]
    pub slot: SlotOptions,
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

    #[arg(long)]
    #[arg(value_delimiter = ',')]
    #[arg(help = "Enables the CORS layer and sets the allowed origins, separated by commas.")]
    pub allowed_origins: Option<Vec<String>>,
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
    #[arg(help = "The maximum number of steps available for the account validation logic.")]
    #[arg(default_value_t = DEFAULT_VALIDATE_MAX_STEPS)]
    pub validate_max_steps: u32,

    #[arg(long)]
    #[arg(help = "The maximum number of steps available for the account execution logic.")]
    #[arg(default_value_t = DEFAULT_INVOKE_MAX_STEPS)]
    pub invoke_max_steps: u32,

    #[arg(long = "eth-gas-price")]
    #[arg(conflicts_with = "genesis")]
    #[arg(help = "The L1 ETH gas price. (denominated in wei)")]
    #[arg(default_value_t = DEFAULT_ETH_L1_GAS_PRICE)]
    pub l1_eth_gas_price: u128,

    #[arg(long = "strk-gas-price")]
    #[arg(conflicts_with = "genesis")]
    #[arg(help = "The L1 STRK gas price. (denominated in fri)")]
    #[arg(default_value_t = DEFAULT_STRK_L1_GAS_PRICE)]
    pub l1_strk_gas_price: u128,
}

#[cfg(feature = "slot")]
#[derive(Debug, Args, Clone)]
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
        let server_config = self.server_config();
        let sequencer_config = self.sequencer_config();
        let starknet_config = self.starknet_config()?;

        // build the node and start it
        let (rpc_handle, backend) =
            katana_node::start(server_config, sequencer_config, starknet_config).await?;

        if !self.silent {
            #[allow(deprecated)]
            let genesis = &backend.config.genesis;
            print_intro(&self, genesis, rpc_handle.addr);
        }

        // Wait until Ctrl + C is pressed, then shutdown
        ctrl_c().await?;
        rpc_handle.handle.stop()?;

        Ok(())
    }

    fn init_logging(&self) -> Result<()> {
        const DEFAULT_LOG_FILTER: &str = "info,executor=trace,forking::backend=trace,server=debug,\
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

    #[allow(deprecated)]
    fn sequencer_config(&self) -> SequencerConfig {
        SequencerConfig {
            block_time: self.block_time,
            no_mining: self.no_mining,
            #[cfg(feature = "messaging")]
            messaging: self.messaging.clone(),
        }
    }

    fn server_config(&self) -> ServerConfig {
        let mut apis = vec![ApiKind::Starknet, ApiKind::Torii, ApiKind::Saya];
        // only enable `katana` API in dev mode
        if self.dev {
            apis.push(ApiKind::Dev);
        }

        ServerConfig {
            apis,
            metrics: self.metrics,
            port: self.server.port,
            host: self.server.host.clone().unwrap_or("0.0.0.0".into()),
            max_connections: self.server.max_connections,
            allowed_origins: self.server.allowed_origins.clone(),
        }
    }

    fn starknet_config(&self) -> Result<StarknetConfig> {
        let genesis = match self.starknet.genesis.clone() {
            Some(genesis) => genesis,
            None => {
                let gas_prices = GasPrices {
                    eth: self.starknet.environment.l1_eth_gas_price,
                    strk: self.starknet.environment.l1_strk_gas_price,
                };

                let accounts = DevAllocationsGenerator::new(self.starknet.total_accounts)
                    .with_seed(parse_seed(&self.starknet.seed))
                    .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
                    .generate();

                let mut genesis = Genesis {
                    gas_prices,
                    sequencer_address: *DEFAULT_SEQUENCER_ADDRESS,
                    ..Default::default()
                };

                #[cfg(feature = "slot")]
                if self.slot.controller {
                    katana_slot_controller::add_controller_account(&mut genesis)?;
                }

                genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));
                genesis
            }
        };

        Ok(StarknetConfig {
            disable_fee: self.starknet.disable_fee,
            disable_validate: self.starknet.disable_validate,
            fork_rpc_url: self.rpc_url.clone(),
            fork_block_number: self.fork_block_number,
            env: Environment {
                chain_id: self.starknet.environment.chain_id,
                invoke_max_steps: self.starknet.environment.invoke_max_steps,
                validate_max_steps: self.starknet.environment.validate_max_steps,
            },
            db_dir: self.db_dir.clone(),
            genesis,
        })
    }
}

fn print_intro(args: &NodeArgs, genesis: &Genesis, address: SocketAddr) {
    let mut accounts = genesis.accounts().peekable();
    let account_class_hash = accounts.peek().map(|e| e.1.class_hash());
    let seed = &args.starknet.seed;

    if args.json_log {
        info!(
            target: LOG_TARGET,
            "{}",
            serde_json::json!({
                "accounts": accounts.map(|a| serde_json::json!(a)).collect::<Vec<_>>(),
                "seed": format!("{}", seed),
                "address": format!("{address}"),
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

        print_genesis_contracts(genesis, account_class_hash);
        print_genesis_accounts(accounts);

        println!(
            r"

ACCOUNTS SEED
=============
{seed}
    "
        );

        let addr = format!(
            "🚀 JSON-RPC server started: {}",
            Style::new().red().apply_to(format!("http://{address}"))
        );

        println!("\n{addr}\n\n",);
    }
}

fn print_genesis_contracts(genesis: &Genesis, account_class_hash: Option<ClassHash>) {
    println!(
        r"
PREDEPLOYED CONTRACTS
==================

| Contract        | Fee Token
| Address         | {}
| Class Hash      | {:#064x}",
        genesis.fee_token.address, genesis.fee_token.class_hash,
    );

    if let Some(ref udc) = genesis.universal_deployer {
        println!(
            r"
| Contract        | Universal Deployer
| Address         | {}
| Class Hash      | {:#064x}",
            udc.address, udc.class_hash
        )
    }

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
    use super::*;

    #[test]
    fn test_starknet_config_default() {
        let args = NodeArgs::parse_from(["katana"]);
        let config = args.starknet_config().unwrap();

        assert!(!config.disable_fee);
        assert!(!config.disable_validate);
        assert_eq!(config.fork_rpc_url, None);
        assert_eq!(config.fork_block_number, None);
        assert_eq!(config.env.chain_id, ChainId::parse("KATANA").unwrap());
        assert_eq!(config.env.invoke_max_steps, DEFAULT_INVOKE_MAX_STEPS);
        assert_eq!(config.env.validate_max_steps, DEFAULT_VALIDATE_MAX_STEPS);
        assert_eq!(config.db_dir, None);
        assert_eq!(config.genesis.gas_prices.eth, DEFAULT_ETH_L1_GAS_PRICE);
        assert_eq!(config.genesis.gas_prices.strk, DEFAULT_STRK_L1_GAS_PRICE);
        assert_eq!(config.genesis.sequencer_address, *DEFAULT_SEQUENCER_ADDRESS);
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
            "--eth-gas-price",
            "10",
            "--strk-gas-price",
            "20",
        ]);
        let config = args.starknet_config().unwrap();

        assert!(config.disable_fee);
        assert!(config.disable_validate);
        assert_eq!(config.env.chain_id, ChainId::GOERLI);
        assert_eq!(config.env.invoke_max_steps, 200);
        assert_eq!(config.env.validate_max_steps, 100);
        assert_eq!(config.db_dir, Some(PathBuf::from("/path/to/db")));
        assert_eq!(config.genesis.gas_prices.eth, 10);
        assert_eq!(config.genesis.gas_prices.strk, 20);
    }
}
