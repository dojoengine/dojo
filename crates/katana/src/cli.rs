use std::path::PathBuf;

use clap::{Args, Parser};
use katana_core::constants::DEFAULT_GAS_PRICE;
use katana_core::starknet::StarknetConfig;
use katana_rpc::config::RpcConfig;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct App {
    #[arg(long)]
    #[arg(help = "Hide the predeployed accounts details.")]
    pub hide_predeployed_accounts: bool,

    #[command(flatten)]
    #[command(next_help_heading = "Server options")]
    pub rpc: RpcOptions,

    #[command(flatten)]
    #[command(next_help_heading = "Starknet options")]
    pub starknet: StarknetOptions,
}

#[derive(Debug, Args, Clone)]
pub struct RpcOptions {
    #[arg(short, long)]
    #[arg(default_value = "5050")]
    #[arg(help = "Port number to listen on.")]
    pub port: u16,
}

#[derive(Debug, Args, Clone)]
pub struct StarknetOptions {
    #[arg(long)]
    #[arg(help = "Specify the seed for randomness of accounts to be predeployed.")]
    pub seed: Option<String>,

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
    #[arg(help = "Block generation on demand via an endpoint.")]
    pub blocks_on_demand: bool,

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
}

impl App {
    pub fn rpc_config(&self) -> RpcConfig {
        RpcConfig { port: self.rpc.port }
    }

    pub fn starknet_config(&self) -> StarknetConfig {
        StarknetConfig {
            total_accounts: self.starknet.total_accounts,
            seed: parse_seed(self.starknet.seed.clone()),
            gas_price: self.starknet.environment.gas_price.unwrap_or(DEFAULT_GAS_PRICE),
            blocks_on_demand: self.starknet.blocks_on_demand,
            account_path: self.starknet.account_path.clone(),
            allow_zero_max_fee: self.starknet.allow_zero_max_fee,
            chain_id: self.starknet.environment.chain_id.clone(),
        }
    }
}

fn parse_seed(seed: Option<String>) -> [u8; 32] {
    seed.map(|seed| {
        let seed = seed.as_bytes();

        if seed.len() >= 32 {
            unsafe { *(seed[..32].as_ptr() as *const [u8; 32]) }
        } else {
            let mut actual_seed = [0u8; 32];
            seed.iter().enumerate().for_each(|(i, b)| actual_seed[i] = *b);
            actual_seed
        }
    })
    .unwrap_or_default()
}
