use env_logger::Env;
use katana_core::sequencer::Sequencer;
use katana_rpc::KatanaRpc;
use log::{error, info};
use std::process::exit;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("starting katana rpc server...");
    match KatanaRpc::new(Sequencer::new()).run().await {
        Ok((addr, server_handle)) => {
            info!("===================================================");
            info!("Katana JSON-RPC Server started: http://{addr}");
            info!("===================================================");

            server_handle.stopped().await;
        }
        Err(err) => {
            error! {"{}", err};
            exit(1);
        }
    };
}
