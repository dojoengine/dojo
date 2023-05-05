use std::{path::PathBuf, process::exit, sync::Arc};

use clap::Parser;
use env_logger::Env;
use katana_core::{sequencer::KatanaSequencer, starknet::StarknetConfig};
use katana_rpc::{config::RpcConfig, KatanaRpc};
use log::error;
use tokio::sync::RwLock;
use yansi::Paint;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config = Cli::parse();
    let rpc_config = config.get_rpc_config();
    let starknet_config = config.get_starknet_config();

    let sequencer = Arc::new(RwLock::new(KatanaSequencer::new(starknet_config)));
    sequencer.write().await.start();

    let predeployed_accounts = sequencer
        .read()
        .await
        .starknet
        .predeployed_accounts
        .display();

    match KatanaRpc::new(sequencer.clone(), rpc_config).run().await {
        Ok((addr, server_handle)) => {
            print_intro(
                predeployed_accounts,
                format!(
                    "🚀 JSON-RPC server started: {}",
                    Paint::red(format!("http://{addr}"))
                ),
            );

            server_handle.stopped().await;
        }
        Err(err) => {
            error! {"{}", err};
            exit(1);
        }
    };
}

#[derive(Parser, Debug)]
struct Cli {
    #[arg(short, long)]
    #[arg(default_value = "5050")]
    #[arg(help = "Port number to listen on.")]
    port: u16,

    #[arg(long)]
    #[arg(default_value = "10")]
    #[arg(help = "Number of pre-funded accounts to generate.")]
    accounts: u8,

    #[arg(long)]
    account_path: Option<PathBuf>,
}

impl Cli {
    fn get_rpc_config(&self) -> RpcConfig {
        RpcConfig { port: self.port }
    }

    fn get_starknet_config(&self) -> StarknetConfig {
        StarknetConfig {
            total_accounts: self.accounts,
            account_path: self.account_path.clone(),
        }
    }
}

fn print_intro(accounts: String, address: String) {
    println!(
        "{}",
        Paint::red(
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
    println!(
        r"        
PREFUNDED ACCOUNTS
==================
{accounts}
"
    );
    println!("\n{address}\n\n");
}
