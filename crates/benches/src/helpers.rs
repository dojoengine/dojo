use std::fs::OpenOptions;
use std::io::Write;

use anyhow::{Context, Result};
use starknet::accounts::{Account, Call, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::Felt;
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;
use tokio::sync::OnceCell;

pub type OwnerAccount = SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>;
#[derive(Debug, Clone)]
pub struct BenchCall(pub &'static str, pub Vec<Felt>);

// Because no calls are actually executed in the benchmark, we can use the same nonce for all of
// them
pub async fn cached_nonce(account: &OwnerAccount) -> Felt {
    static NONCE: OnceCell<Felt> = OnceCell::const_new();

    *NONCE.get_or_init(|| async { account.get_nonce().await.unwrap() }).await
}

pub fn log(name: &str, gas: u64, calldata: &str) {
    let mut file = OpenOptions::new().create(true).append(true).open("gas_usage.txt").unwrap();

    let mut calldata = String::from(calldata);
    if !calldata.is_empty() {
        calldata = String::from("\tcalldata: ") + &calldata
    }

    writeln!(file, "{}\tfee: {}{calldata}", name, gas).unwrap();
    file.flush().unwrap();
}

pub fn parse_calls(calls: Vec<BenchCall>, to: Felt) -> Vec<Call> {
    calls
        .into_iter()
        .map(|BenchCall(name, calldata)| Call {
            to,
            selector: get_selector_from_name(name).context("Failed to get selector").unwrap(),
            calldata,
        })
        .collect()
}

pub async fn estimate_calls(account: &OwnerAccount, calls: Vec<Call>) -> Result<Felt> {
    let fee = account
        .execute_v1(calls)
        .nonce(cached_nonce(account).await)
        .estimate_fee()
        .await
        .context("Failed to estimate fee")?;

    Ok(fee.gas_consumed)
}

pub async fn execute_calls(account: OwnerAccount, calls: Vec<Call>, nonce: Felt) -> Result<()> {
    account.execute_v1(calls).nonce(nonce).send().await.context("Failed to execute").unwrap();

    Ok(())
}
