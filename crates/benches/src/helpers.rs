use std::fs::OpenOptions;
use std::io::Write;

use anyhow::{Context, Result};
use reqwest::Url;
use starknet::accounts::{Account, Call, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, FieldElement};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};
use tokio::sync::OnceCell;

use crate::{BenchCall, OwnerAccount, ACCOUNT_ADDRESS, CONTRACT, KATANA_ENDPOINT, PRIVATE_KEY};

pub async fn chain_id() -> FieldElement {
    static CHAIN_ID: OnceCell<FieldElement> = OnceCell::const_new();

    *CHAIN_ID
        .get_or_init(|| async {
            let provider = provider();
            provider.chain_id().await.unwrap()
        })
        .await
}

// Because no calls are actually executed in the benchmark, we can use the same nonce for all of
// them
pub async fn cached_nonce() -> FieldElement {
    static NONCE: OnceCell<FieldElement> = OnceCell::const_new();

    *NONCE
        .get_or_init(|| async {
            let account = account().await;
            account.get_nonce().await.unwrap()
        })
        .await
}

pub async fn account() -> OwnerAccount {
    let signer = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
        FieldElement::from_hex_be(PRIVATE_KEY).unwrap(),
    ));
    let address = FieldElement::from_hex_be(ACCOUNT_ADDRESS).unwrap();
    let mut account = SingleOwnerAccount::new(
        provider(),
        signer,
        address,
        chain_id().await,
        ExecutionEncoding::Legacy,
    );
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    account
}

pub fn provider() -> JsonRpcClient<HttpTransport> {
    let url = Url::parse(KATANA_ENDPOINT).expect("Invalid Katana endpoint");
    JsonRpcClient::new(HttpTransport::new(url))
}

pub fn log(name: &str, gas: u64, calldata: &str) {
    let mut file =
        OpenOptions::new().create(true).write(true).append(true).open("gas_usage.txt").unwrap();

    let mut calldata = String::from(calldata);
    if !calldata.is_empty() {
        calldata = String::from("\tcalldata: ") + &calldata
    }

    writeln!(file, "{}\tfee: {}{calldata}", name, gas).unwrap();
    file.flush().unwrap();
}

pub fn parse_calls(entrypoints_and_calldata: Vec<BenchCall>) -> Vec<Call> {
    entrypoints_and_calldata
        .into_iter()
        .map(|BenchCall(name, calldata)| Call {
            to: *CONTRACT,
            selector: get_selector_from_name(name).context("Failed to get selector").unwrap(),
            calldata,
        })
        .collect()
}

pub async fn estimate_calls(calls: Vec<Call>) -> Result<u64> {
    let fee = account()
        .await
        .execute(calls)
        .nonce(cached_nonce().await)
        .estimate_fee()
        .await
        .context("Failed to estimate fee")
        .unwrap();

    Ok(fee.gas_consumed)
}

pub async fn execute_calls(calls: Vec<Call>, nonce: FieldElement) -> Result<()> {
    account().await.execute(calls).nonce(nonce).send().await.context("Failed to execute").unwrap();

    Ok(())
}
