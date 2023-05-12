use std::path::PathBuf;

use clap::Parser;
use katana_core::starknet::StarknetConfig;
use katana_rpc::config::RpcConfig;

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(short, long)]
    #[arg(default_value = "5050")]
    #[arg(help = "Port number to listen on.")]
    pub port: u16,

    #[arg(long)]
    #[arg(help = "Allow transaction max fee to be zero.")]
    pub allow_max_fee_zero: bool,

    #[arg(long)]
    #[arg(help = "Specify the seed for randomness of accounts to be predeployed.")]
    pub seed: Option<String>,

    #[arg(long)]
    #[arg(default_value = "10")]
    #[arg(help = "Number of pre-funded accounts to generate.")]
    pub accounts: u8,

    #[arg(long)]
    #[arg(help = "Prevents from printing the predeployed accounts details.")]
    pub hide_predeployed_accounts: bool,

    #[arg(long)]
    #[arg(help = "Block generation on demand via an endpoint.")]
    pub blocks_on_demand: bool,

    #[arg(long)]
    #[arg(help = "The account implementation for the predeployed accounts.")]
    #[arg(
        long_help = "Specify the account implementation to be used for the predeployed accounts; should be a path to the
    compiled JSON artifact."
    )]
    pub account_class: Option<PathBuf>,
}

impl Cli {
    pub fn rpc_config(&self) -> RpcConfig {
        RpcConfig { port: self.port }
    }

    pub fn starknet_config(&self) -> StarknetConfig {
        StarknetConfig {
            total_accounts: self.accounts,
            seed: parse_seed(self.seed.clone()),
            block_on_demand: self.blocks_on_demand,
            account_path: self.account_class.clone(),
            allow_zero_max_fee: self.allow_max_fee_zero,
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
            seed.iter()
                .enumerate()
                .for_each(|(i, b)| actual_seed[i] = *b);
            actual_seed
        }
    })
    .unwrap_or_default()
}
