use std::process::exit;
use std::sync::Arc;
use std::{fs, io};

use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use console::Style;
use env_logger::Env;
use katana_core::sequencer::{KatanaSequencer, Sequencer};
use katana_rpc::{spawn, KatanaApi, NodeHandle, StarknetApi};
use log::{error, info};
use tokio::signal::ctrl_c;

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
                let accounts = sequencer
                    .backend
                    .accounts
                    .iter()
                    .map(|a| format!("{a}"))
                    .collect::<Vec<_>>()
                    .join("\n");

                print_intro(
                    accounts,
                    config.starknet.seed.clone(),
                    format!(
                        "🚀 JSON-RPC server started: {}",
                        Style::new().red().apply_to(format!("http://{addr}"))
                    ),
                );
            }

            sequencer.start().await;

            // Wait until Ctrl + C is pressed, then shutdown
            ctrl_c().await.unwrap();
            shutdown_handler(sequencer.clone(), config).await;
            handle.stop().unwrap();
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

pub async fn shutdown_handler(sequencer: Arc<impl Sequencer>, config: KatanaArgs) {
    if let Some(path) = config.dump_state {
        info!("Dumping state on shutdown");
        let state = (*sequencer).backend().dump_state().await;
        if let Ok(state) = state {
            match fs::write(path.clone(), state) {
                Ok(_) => {
                    info!("Successfully dumped state")
                }
                Err(_) => {
                    error!("Failed to write state dump to {:?}", path)
                }
            };
        } else {
            error!("Failed to fetch state dump.")
        }
    };
}
