use env_logger::Env;
use katana_core::{sequencer::KatanaSequencer, state::ACCOUNT_CONTRACT_CLASS_HASH};
use katana_rpc::KatanaRpc;
use log::{error, info};
use starknet_api::{
    core::ClassHash,
    hash::StarkFelt,
    stark_felt,
    transaction::{Calldata, ContractAddressSalt, TransactionSignature, TransactionVersion},
};
use std::process::exit;

pub const DEFAULT_BALANCE: u64 = 1000000 * 100000000000; // 1000000 * min_gas_price.

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let sequencer = KatanaSequencer::new();

    let (tx_hash, account_address) = sequencer
        .drip_and_deploy_account(
            ClassHash(stark_felt!(ACCOUNT_CONTRACT_CLASS_HASH)),
            TransactionVersion(stark_felt!(1)),
            ContractAddressSalt::default(),
            Calldata::default(),
            TransactionSignature::default(),
            DEFAULT_BALANCE,
        )
        .unwrap_or_else(|err| {
            error! {"{}", err};
            exit(1);
        });

    info!(
        "Deployed test account at: {} with txn: {}",
        account_address.0.key(),
        tx_hash.0
    );

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
