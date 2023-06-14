use std::process::exit;
use std::sync::Arc;

use clap::Parser;
use env_logger::Env;
use katana_core::sequencer::KatanaSequencer;
use katana_rpc::KatanaNodeRpc;
use log::error;
use yansi::Paint;

mod cli;

use cli::App;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or(
        "info,katana_rpc=debug,katana_core=debug,blockifier=off,jsonrpsee_server=off,hyper=off",
    ))
    .init();

    let config = App::parse();

    let rpc_config = config.rpc_config();
    let sequencer_config = config.sequencer_config();
    let starknet_config = config.starknet_config();

    let sequencer = Arc::new(KatanaSequencer::new(sequencer_config, starknet_config));

    let predeployed_accounts = if config.hide_predeployed_accounts {
        None
    } else {
        Some(sequencer.starknet.read().await.predeployed_accounts.display())
    };

    match KatanaNodeRpc::new(sequencer.clone(), rpc_config).run().await {
        Ok((addr, server_handle)) => {
            print_intro(
                predeployed_accounts,
                config.starknet.seed,
                format!("🚀 JSON-RPC server started: {}", Paint::red(format!("http://{addr}"))),
            );

            sequencer.start().await;
            server_handle.stopped().await;
        }
        Err(err) => {
            error! {"{}", err};
            exit(1);
        }
    };
}

fn print_intro(accounts: Option<String>, seed: Option<String>, address: String) {
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

    if let Some(accounts) = accounts {
        println!(
            r"        
PREFUNDED ACCOUNTS
==================
{accounts}
    "
        );
    }

    if let Some(seed) = seed {
        println!(
            r"
ACCOUNTS SEED
=============
{seed}
    "
        );
    }

    println!("\n{address}\n\n");
}
