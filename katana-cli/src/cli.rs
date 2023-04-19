use env_logger::Env;
use katana_core::sequencer::KatanaSequencer;
use katana_rpc::KatanaRpc;
use log::{error, info};
use starknet_api::transaction::ContractAddressSalt;
use std::process::exit;

pub const DEFAULT_BALANCE: u64 = 1000000 * 100000000000; // 1000000 * min_gas_price.

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let mut sequencer = KatanaSequencer::new();
    let account_address = sequencer
        .deploy_account(ContractAddressSalt::default(), DEFAULT_BALANCE)
        .unwrap_or_else(|err| {
            error! {"{}", err};
            exit(1);
        });

    info!("deployed test account at: {}", account_address.0.key());

    info!("starting katana rpc server...");
    match KatanaRpc::new(sequencer).run().await {
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
