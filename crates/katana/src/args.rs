use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap_complete::Shell;
use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::constants::{
    DEFAULT_GAS_PRICE, DEFAULT_INVOKE_MAX_STEPS, DEFAULT_VALIDATE_MAX_STEPS,
};
use katana_core::db::serde::state::SerializableState;
use katana_core::sequencer::SequencerConfig;
use katana_rpc::config::ServerConfig;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct KatanaArgs {
    #[arg(long)]
    #[arg(help = "Don't print anything on startup.")]
    pub silent: bool,

    #[arg(long)]
    #[arg(conflicts_with = "block_time")]
    #[arg(help = "Disable auto and interval mining, and mine on demand instead.")]
    pub no_mining: bool,

    #[arg(short, long)]
    #[arg(value_name = "SECONDS")]
    #[arg(help = "Block time in seconds for interval mining.")]
    pub block_time: Option<u64>,

    #[arg(long)]
    #[arg(value_name = "PATH")]
    #[arg(help = "Dump the state of chain on exit to the given file.")]
    #[arg(long_help = "Dump the state of chain on exit to the given file. \
                       If the value is a directory, the state will be written to `<PATH>/state.bin`.")]
    pub dump_state: Option<PathBuf>,

    #[arg(long)]
    #[arg(hide = true)]
    #[arg(value_name = "PATH")]
    #[arg(value_parser = SerializableState::parse)]
    #[arg(help = "Initialize the chain from a previously saved state snapshot.")]
    pub load_state: Option<SerializableState>,

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
    pub total_accounts: u8,

    #[arg(value_name = "PATH")]
    #[arg(long = "account-class")]
    #[arg(help = "The account implementation for the predeployed accounts.")]
    #[arg(long_help = "Specify the account implementation to be used for the predeployed \
                       accounts; should be a path to the compiled JSON artifact.")]
    pub account_path: Option<PathBuf>,

    #[arg(long)]
    #[arg(help = "Allow transaction max fee to be zero.")]
    pub allow_zero_max_fee: bool,

    #[command(flatten)]
    #[command(next_help_heading = "Environment options")]
    pub environment: EnvironmentOptions,
}

#[derive(Debug, Args, Clone)]
pub struct EnvironmentOptions {
    #[arg(long)]
    #[arg(help = "The chain ID.")]
    #[arg(default_value = "KATANA")]
    pub chain_id: String,

    #[arg(long)]
    #[arg(help = "The gas price.")]
    pub gas_price: Option<u128>,

    #[arg(long)]
    #[arg(help = "The maximum number of steps available for the account validation logic.")]
    pub validate_max_steps: Option<u32>,

    #[arg(long)]
    #[arg(help = "The maximum number of steps available for the account execution logic.")]
    pub invoke_max_steps: Option<u32>,
}

impl KatanaArgs {
    pub fn sequencer_config(&self) -> SequencerConfig {
        SequencerConfig { block_time: self.block_time }
    }

    pub fn server_config(&self) -> ServerConfig {
        ServerConfig {
            port: self.server.port,
            host: self.server.host.clone().unwrap_or("0.0.0.0".into()),
        }
    }

    pub fn starknet_config(&self) -> StarknetConfig {
        StarknetConfig {
            total_accounts: self.starknet.total_accounts,
            seed: parse_seed(&self.starknet.seed),
            account_path: self.starknet.account_path.clone(),
            allow_zero_max_fee: self.starknet.allow_zero_max_fee,
            auto_mine: self.block_time.is_none() && !self.no_mining,
            init_state: self.load_state.clone(),
            env: Environment {
                chain_id: self.starknet.environment.chain_id.clone(),
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
        }
    }
}

fn parse_seed(seed: &str) -> [u8; 32] {
    let seed = seed.as_bytes();

    if seed.len() >= 32 {
        unsafe { *(seed[..32].as_ptr() as *const [u8; 32]) }
    } else {
        let mut actual_seed = [0u8; 32];
        seed.iter().enumerate().for_each(|(i, b)| actual_seed[i] = *b);
        actual_seed
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn default_block_context_from_args() {
        let args = KatanaArgs::parse_from(["katana"]);
        let block_context = args.starknet_config().block_context();
        assert_eq!(block_context.gas_price, DEFAULT_GAS_PRICE);
        assert_eq!(block_context.chain_id.0, "KATANA".to_string());
        assert_eq!(block_context.validate_max_n_steps, DEFAULT_VALIDATE_MAX_STEPS);
        assert_eq!(block_context.invoke_tx_max_n_steps, DEFAULT_INVOKE_MAX_STEPS);
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

        let block_context = args.starknet_config().block_context();

        assert_eq!(block_context.gas_price, 10);
        assert_eq!(block_context.chain_id.0, "SN_GOERLI".to_string());
        assert_eq!(block_context.validate_max_n_steps, 100);
        assert_eq!(block_context.invoke_tx_max_n_steps, 200);
    }
}
