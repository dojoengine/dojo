//! Options related to the CLI and the configuration file parsing.
//!
//! The clap args are first parsed, then the configuration file is parsed.
//! If no configuration file is provided, the default values are used form the clap args.
//! If a configuration file is provided, the values are merged with the clap args, however, the clap
//! args keep the precedence.
//!
//! Currently, the merge is made at the top level of the commands.

#[cfg(feature = "server")]
use std::net::IpAddr;

use clap::Args;
use katana_node::config::execution::{DEFAULT_INVOCATION_MAX_STEPS, DEFAULT_VALIDATION_MAX_STEPS};
#[cfg(feature = "server")]
use katana_node::config::metrics::{DEFAULT_METRICS_ADDR, DEFAULT_METRICS_PORT};
#[cfg(feature = "server")]
use katana_node::config::rpc::{RpcModulesList, DEFAULT_RPC_MAX_PROOF_KEYS};
#[cfg(feature = "server")]
use katana_node::config::rpc::{
    DEFAULT_RPC_ADDR, DEFAULT_RPC_MAX_EVENT_PAGE_SIZE, DEFAULT_RPC_PORT,
};
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::chain::ChainId;
use katana_primitives::genesis::Genesis;
#[cfg(feature = "server")]
use katana_rpc::cors::HeaderValue;
use serde::{Deserialize, Serialize};
use url::Url;

#[cfg(feature = "server")]
use crate::utils::{deserialize_cors_origins, serialize_cors_origins};
use crate::utils::{parse_block_hash_or_number, parse_genesis, LogFormat};

const DEFAULT_DEV_SEED: &str = "0";
const DEFAULT_DEV_ACCOUNTS: u16 = 10;

#[cfg(feature = "server")]
#[derive(Debug, Args, Clone, Serialize, Deserialize, PartialEq)]
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

#[cfg(feature = "server")]
impl Default for MetricsOptions {
    fn default() -> Self {
        MetricsOptions {
            metrics: false,
            metrics_addr: DEFAULT_METRICS_ADDR,
            metrics_port: DEFAULT_METRICS_PORT,
        }
    }
}

#[cfg(feature = "server")]
#[derive(Debug, Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "Server options")]
pub struct ServerOptions {
    /// HTTP-RPC server listening interface.
    #[arg(long = "http.addr", value_name = "ADDRESS")]
    #[arg(default_value_t = DEFAULT_RPC_ADDR)]
    #[serde(default = "default_http_addr")]
    pub http_addr: IpAddr,

    /// HTTP-RPC server listening port.
    #[arg(long = "http.port", value_name = "PORT")]
    #[arg(default_value_t = DEFAULT_RPC_PORT)]
    #[serde(default = "default_http_port")]
    pub http_port: u16,

    /// Comma separated list of domains from which to accept cross origin requests.
    #[arg(long = "http.cors_origins")]
    #[arg(value_delimiter = ',')]
    #[serde(
        default,
        serialize_with = "serialize_cors_origins",
        deserialize_with = "deserialize_cors_origins"
    )]
    pub http_cors_origins: Vec<HeaderValue>,

    /// API's offered over the HTTP-RPC interface.
    #[arg(long = "http.api", value_name = "MODULES")]
    #[arg(value_parser = RpcModulesList::parse)]
    #[serde(default)]
    pub http_modules: Option<RpcModulesList>,

    /// Maximum number of concurrent connections allowed.
    #[arg(long = "rpc.max-connections", value_name = "MAX")]
    pub max_connections: Option<u32>,

    /// Maximum request body size (in bytes).
    #[arg(long = "rpc.max-request-body-size", value_name = "SIZE")]
    pub max_request_body_size: Option<u32>,

    /// Maximum response body size (in bytes).
    #[arg(long = "rpc.max-response-body-size", value_name = "SIZE")]
    pub max_response_body_size: Option<u32>,

    /// Maximum page size for event queries.
    #[arg(long = "rpc.max-event-page-size", value_name = "SIZE")]
    #[arg(default_value_t = DEFAULT_RPC_MAX_EVENT_PAGE_SIZE)]
    #[serde(default = "default_page_size")]
    pub max_event_page_size: u64,

    /// Maximum keys for requesting storage proofs.
    #[arg(long = "rpc.max-proof-keys", value_name = "SIZE")]
    #[arg(default_value_t = DEFAULT_RPC_MAX_PROOF_KEYS)]
    #[serde(default = "default_proof_keys")]
    pub max_proof_keys: u64,
}

#[cfg(feature = "server")]
impl Default for ServerOptions {
    fn default() -> Self {
        ServerOptions {
            http_addr: DEFAULT_RPC_ADDR,
            http_port: DEFAULT_RPC_PORT,
            http_cors_origins: Vec::new(),
            http_modules: Some(RpcModulesList::default()),
            max_event_page_size: DEFAULT_RPC_MAX_EVENT_PAGE_SIZE,
            max_proof_keys: DEFAULT_RPC_MAX_PROOF_KEYS,
            max_connections: None,
            max_request_body_size: None,
            max_response_body_size: None,
        }
    }
}

#[derive(Debug, Args, Clone, Serialize, Deserialize, Default, PartialEq)]
#[command(next_help_heading = "Starknet options")]
pub struct StarknetOptions {
    #[command(flatten)]
    #[serde(rename = "env")]
    pub environment: EnvironmentOptions,

    #[arg(long)]
    #[arg(value_parser = parse_genesis)]
    #[arg(conflicts_with_all(["seed", "total_accounts", "chain"]))]
    pub genesis: Option<Genesis>,
}

impl StarknetOptions {
    pub fn merge(&mut self, other: Option<&Self>) {
        if let Some(other) = other {
            self.environment.merge(Some(&other.environment));

            if self.genesis.is_none() {
                self.genesis = other.genesis.clone();
            }
        }
    }
}

#[derive(Debug, Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "Environment options")]
pub struct EnvironmentOptions {
    /// The chain ID.
    ///
    /// The chain ID. If a raw hex string (`0x` prefix) is provided, then it'd
    /// used as the actual chain ID. Otherwise, it's represented as the raw
    /// ASCII values. It must be a valid Cairo short string.
    #[arg(long, conflicts_with = "chain")]
    #[arg(value_parser = ChainId::parse)]
    #[serde(default)]
    pub chain_id: Option<ChainId>,

    /// The maximum number of steps available for the account validation logic.
    #[arg(long)]
    #[arg(default_value_t = DEFAULT_VALIDATION_MAX_STEPS)]
    #[serde(default = "default_validate_max_steps")]
    pub validate_max_steps: u32,

    /// The maximum number of steps available for the account execution logic.
    #[arg(long)]
    #[arg(default_value_t = DEFAULT_INVOCATION_MAX_STEPS)]
    #[serde(default = "default_invoke_max_steps")]
    pub invoke_max_steps: u32,
}

impl Default for EnvironmentOptions {
    fn default() -> Self {
        EnvironmentOptions {
            validate_max_steps: DEFAULT_VALIDATION_MAX_STEPS,
            invoke_max_steps: DEFAULT_INVOCATION_MAX_STEPS,
            chain_id: None,
        }
    }
}

impl EnvironmentOptions {
    pub fn merge(&mut self, other: Option<&Self>) {
        if let Some(other) = other {
            if self.validate_max_steps == DEFAULT_VALIDATION_MAX_STEPS {
                self.validate_max_steps = other.validate_max_steps;
            }

            if self.invoke_max_steps == DEFAULT_INVOCATION_MAX_STEPS {
                self.invoke_max_steps = other.invoke_max_steps;
            }
        }
    }
}

#[derive(Debug, Args, Clone, Serialize, Deserialize, PartialEq)]
#[command(next_help_heading = "Development options")]
#[serde(rename = "dev")]
pub struct DevOptions {
    /// Enable development mode.
    #[arg(long)]
    #[serde(default)]
    pub dev: bool,

    /// Specify the seed for randomness of accounts to be predeployed.
    #[arg(requires = "dev")]
    #[arg(long = "dev.seed", default_value = DEFAULT_DEV_SEED)]
    #[serde(default = "default_seed")]
    pub seed: String,

    /// Number of pre-funded accounts to generate.
    #[arg(requires = "dev")]
    #[arg(long = "dev.accounts", value_name = "NUM")]
    #[arg(default_value_t = DEFAULT_DEV_ACCOUNTS)]
    #[serde(default = "default_accounts")]
    pub total_accounts: u16,

    /// Disable charging fee when executing transactions.
    #[arg(requires = "dev")]
    #[arg(long = "dev.no-fee")]
    #[serde(default)]
    pub no_fee: bool,

    /// Disable account validation when executing transactions.
    ///
    /// Skipping the transaction sender's account validation function.
    #[arg(requires = "dev")]
    #[arg(long = "dev.no-account-validation")]
    #[serde(default)]
    pub no_account_validation: bool,
}

impl Default for DevOptions {
    fn default() -> Self {
        DevOptions {
            dev: false,
            seed: DEFAULT_DEV_SEED.to_string(),
            total_accounts: DEFAULT_DEV_ACCOUNTS,
            no_fee: false,
            no_account_validation: false,
        }
    }
}

impl DevOptions {
    pub fn merge(&mut self, other: Option<&Self>) {
        if let Some(other) = other {
            if !self.dev {
                self.dev = other.dev;
            }

            if self.seed == DEFAULT_DEV_SEED {
                self.seed = other.seed.clone();
            }

            if self.total_accounts == DEFAULT_DEV_ACCOUNTS {
                self.total_accounts = other.total_accounts;
            }

            if !self.no_fee {
                self.no_fee = other.no_fee;
            }

            if !self.no_account_validation {
                self.no_account_validation = other.no_account_validation;
            }
        }
    }
}

#[derive(Debug, Args, Clone, Serialize, Deserialize, Default, PartialEq)]
#[command(next_help_heading = "Forking options")]
pub struct ForkingOptions {
    /// The RPC URL of the network to fork from.
    ///
    /// This will operate Katana in forked mode. Continuing from the tip of the forked network, or
    /// at a specific block if `fork.block` is provided.
    #[arg(long = "fork.provider", value_name = "URL", conflicts_with = "genesis")]
    pub fork_provider: Option<Url>,

    /// Fork the network at a specific block id, can either be a hash (0x-prefixed) or a block
    /// number.
    #[arg(long = "fork.block", value_name = "BLOCK", requires = "fork_provider")]
    #[arg(value_parser = parse_block_hash_or_number)]
    pub fork_block: Option<BlockHashOrNumber>,
}

#[derive(Debug, Args, Clone, Serialize, Deserialize, Default, PartialEq)]
#[command(next_help_heading = "Logging options")]
pub struct LoggingOptions {
    /// Log format to use
    #[arg(long = "log.format", value_name = "FORMAT")]
    #[arg(default_value_t = LogFormat::Full)]
    pub log_format: LogFormat,
}

#[derive(Debug, Args, Clone, Serialize, Deserialize, Default, PartialEq)]
#[command(next_help_heading = "Gas Price Oracle Options")]
pub struct GasPriceOracleOptions {
    /// The L1 ETH gas price. (denominated in wei)
    #[arg(long = "gpo.l1-eth-gas-price", value_name = "WEI")]
    #[arg(default_value_t = 0)]
    #[serde(serialize_with = "cainome_cairo_serde::serialize_as_hex")]
    #[serde(deserialize_with = "cainome_cairo_serde::deserialize_from_hex")]
    pub l1_eth_gas_price: u128,

    /// The L1 STRK gas price. (denominated in fri)
    #[arg(long = "gpo.l1-strk-gas-price", value_name = "FRI")]
    #[arg(default_value_t = 0)]
    #[serde(serialize_with = "cainome_cairo_serde::serialize_as_hex")]
    #[serde(deserialize_with = "cainome_cairo_serde::deserialize_from_hex")]
    pub l1_strk_gas_price: u128,

    /// The L1 ETH data gas price. (denominated in wei)
    #[arg(long = "gpo.l1-eth-data-gas-price", value_name = "WEI")]
    #[arg(default_value_t = 0)]
    #[serde(serialize_with = "cainome_cairo_serde::serialize_as_hex")]
    #[serde(deserialize_with = "cainome_cairo_serde::deserialize_from_hex")]
    pub l1_eth_data_gas_price: u128,

    /// The L1 STRK data gas price. (denominated in fri)
    #[arg(long = "gpo.l1-strk-data-gas-price", value_name = "FRI")]
    #[arg(default_value_t = 0)]
    #[serde(serialize_with = "cainome_cairo_serde::serialize_as_hex")]
    #[serde(deserialize_with = "cainome_cairo_serde::deserialize_from_hex")]
    pub l1_strk_data_gas_price: u128,
}

#[cfg(feature = "slot")]
#[derive(Debug, Args, Clone, Serialize, Deserialize, Default, PartialEq)]
#[command(next_help_heading = "Slot options")]
pub struct SlotOptions {
    #[arg(hide = true)]
    #[arg(long = "slot.controller")]
    pub controller: bool,
}

// ** Default functions to setup serde of the configuration file **
fn default_seed() -> String {
    DEFAULT_DEV_SEED.to_string()
}

fn default_accounts() -> u16 {
    DEFAULT_DEV_ACCOUNTS
}

fn default_validate_max_steps() -> u32 {
    DEFAULT_VALIDATION_MAX_STEPS
}

fn default_invoke_max_steps() -> u32 {
    DEFAULT_INVOCATION_MAX_STEPS
}

#[cfg(feature = "server")]
fn default_http_addr() -> IpAddr {
    DEFAULT_RPC_ADDR
}

#[cfg(feature = "server")]
fn default_http_port() -> u16 {
    DEFAULT_RPC_PORT
}

#[cfg(feature = "server")]
fn default_page_size() -> u64 {
    DEFAULT_RPC_MAX_EVENT_PAGE_SIZE
}

#[cfg(feature = "server")]
fn default_proof_keys() -> u64 {
    katana_node::config::rpc::DEFAULT_RPC_MAX_PROOF_KEYS
}

#[cfg(feature = "server")]
fn default_metrics_addr() -> IpAddr {
    DEFAULT_METRICS_ADDR
}

#[cfg(feature = "server")]
fn default_metrics_port() -> u16 {
    DEFAULT_METRICS_PORT
}
