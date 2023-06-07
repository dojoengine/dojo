use std::process::exit;
use std::sync::Arc;

use clap::Parser;
use env_logger::Env;
use katana_core::sequencer::KatanaSequencer;
use katana_core::starknet::serializable::SerializableState;
use katana_rpc::KatanaNodeRpc;
use log::error;
use tokio_util::sync::CancellationToken;
use yansi::Paint;

mod cli;

use cli::App;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config = App::parse();
    let rpc_config = config.rpc_config();
    let starknet_config = config.starknet_config();

    let dump_path = config.dump_path();
    let state_path = config.state_path();
    let dump_interval = config.dump_interval();

    let sequencer = match state_path {
        None => Arc::new(KatanaSequencer::new(starknet_config)),
        Some(path) => Arc::new(
            KatanaSequencer::new_from_dump(starknet_config, &path)
                .expect("Failed to load KatanaSequencer from dump"),
        ),
    };

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

            // If dump_path is set, dump the state every dump_interval seconds.
            if let Some(path) = dump_path {
                // Create a cancellation token to check when the
                // server stops.
                // https://docs.rs/tokio-util/0.7.8/tokio_util/sync/struct.CancellationToken.html
                let cancellation_token = Arc::new(CancellationToken::new());
                let dump_task_cancellation_token = cancellation_token.clone();
                let dump_task = tokio::spawn(async move {
                    let mut interval =
                        tokio::time::interval(tokio::time::Duration::from_secs(dump_interval));
                    loop {
                        interval.tick().await;
                        if dump_task_cancellation_token.is_cancelled() {
                            break;
                        }
                        // Dump the state.
                        sequencer
                            .starknet
                            .read()
                            .await
                            .dump_state(&path)
                            .expect("Failed to dump state");
                    }
                });
                // Create a cancellation token that will be used to cancel the dump task when the
                // server stops.
                let server_task_cancellation_token = cancellation_token.clone();
                let server_task = tokio::spawn(async move {
                    server_handle.stopped().await;
                    server_task_cancellation_token.cancel();
                });

                // Wait for the dump task and the server task to finish.
                let _ = tokio::try_join!(dump_task, server_task);
                cancellation_token.cancel();
            }
            // If dump_state is not set, just wait for the server to stop.
            else {
                server_handle.stopped().await;
            }
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
