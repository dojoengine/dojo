use std::io;
use std::process::exit;
use std::sync::Arc;

use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use env_logger::Env;
use katana_core::sequencer::KatanaSequencer;
use katana_rpc::{spawn, KatanaApi, NodeHandle, StarknetApi};
use log::error;
use yansi::Paint;

mod args;

use args::Commands::Completions;
use args::KatanaArgs;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or(
        "info,katana_rpc=debug,katana_core=trace,blockifier=off,jsonrpsee_server=off,hyper=off",
    ))
    .init();

    let config = KatanaArgs::parse();
    if let Some(command) = config.command {
        match command {
            Completions { shell } => {
                print_completion(shell);
                return;
            }
        }
    }

    let server_config = config.server_config();
    let sequencer_config = config.sequencer_config();
    let starknet_config = config.starknet_config();

    let sequencer = Arc::new(KatanaSequencer::new(sequencer_config, starknet_config));
    let starknet_api = StarknetApi::new(sequencer.clone());
    let katana_api = KatanaApi::new(sequencer.clone());

    match spawn(katana_api, starknet_api, server_config).await {
        Ok(NodeHandle { addr, handle, .. }) => {
            if !config.silent {
                let accounts = sequencer.backend.predeployed_accounts.display();

                print_intro(
                    accounts,
                    config.starknet.seed,
                    format!("🚀 JSON-RPC server started: {}", Paint::red(format!("http://{addr}"))),
                );
            }

            sequencer.start().await;
            handle.stopped().await;
        }
        Err(err) => {
            error! {"{}", err};
            exit(1);
        }
    };
}

fn print_completion(shell: Shell) {
    let mut command = KatanaArgs::command();
    let name = command.get_name().to_string();
    generate(shell, &mut command, name, &mut io::stdout());
}

fn print_intro(accounts: String, seed: String, address: String) {
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

    println!(
        r"
ACCOUNTS SEED
=============
{seed}
    "
    );

    println!("\n{address}\n\n");
}
