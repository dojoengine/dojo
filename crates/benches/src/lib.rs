#[cfg(test)]
#[macro_use]
extern crate lazy_static;

use std::sync::Once;

use anyhow::{Context, Result};
use futures::executor::block_on;
use lazy_static::lazy_static;
use reqwest::Url;
use starknet::accounts::{Account, Call, ConnectedAccount, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, FieldElement};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};
use tokio::runtime::Runtime;
use tokio::sync::OnceCell;
use tracing_subscriber::fmt;

type OwnerAccount = SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>;

const KATANA_ENDPOINT: &str = "http://localhost:6969";
const CONTRACT_ADDRESS: &str = "0x6c27e3b47f88abca376261ad4f0ffbe3461b9d08477f9e10953829603184e13";

const ACCOUNT_ADDRESS: &str = "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973";
const PRIVATE_KEY: &str = "0x1800000000300000180000000000030000000000003006001800006600";

lazy_static! {
    static ref CONTRACT: FieldElement = FieldElement::from_hex_be(CONTRACT_ADDRESS).unwrap();
    static ref RUNTIME: Runtime = Runtime::new().unwrap();
}

async fn chain_id() -> FieldElement {
    static CHAIN_ID: OnceCell<FieldElement> = OnceCell::const_new();

    *CHAIN_ID
        .get_or_init(|| async {
            let provider = provider();
            provider.chain_id().await.unwrap()
        })
        .await
}

// Because no calls are actually executed in the benchmark, we can use the same nonce for all of them
async fn nonce() -> FieldElement {
    static NONCE: OnceCell<FieldElement> = OnceCell::const_new();

    *NONCE
        .get_or_init(|| async {
            let account = account().await;
            account.get_nonce().await.unwrap()
        })
        .await
}

fn logging() {
    static NONCE: Once = Once::new();

    NONCE.call_once(|| {
        let subscriber = fmt::Subscriber::builder()
            .with_max_level(tracing::Level::INFO) // Set the maximum log level
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set the global tracing subscriber");
    });
}

async fn account() -> OwnerAccount {
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

fn provider() -> JsonRpcClient<HttpTransport> {
    let url = Url::parse(KATANA_ENDPOINT).expect("Invalid Katana endpoint");
    JsonRpcClient::new(HttpTransport::new(url))
}

type EntrypointsAndCalldata = Vec<(&'static str, Vec<FieldElement>)>;

pub fn execute(entrypoints_and_calldata: EntrypointsAndCalldata) -> Result<u64> {
    logging();
    let calls = parse_calls(entrypoints_and_calldata);
    let _rt = RUNTIME.enter();
    block_on(async move { execute_calls(calls).await })
}

pub async fn execute_async(entrypoints_and_calldata: EntrypointsAndCalldata) -> Result<u64> {
    logging();
    let calls = parse_calls(entrypoints_and_calldata);
    execute_calls(calls).await
}

fn parse_calls(entrypoints_and_calldata: EntrypointsAndCalldata) -> Vec<Call> {
    entrypoints_and_calldata
        .into_iter()
        .map(|(name, calldata)| Call {
            to: *CONTRACT,
            selector: get_selector_from_name(name).context("Failed to get selector").unwrap(),
            calldata,
        })
        .collect()
}

async fn execute_calls(calls: Vec<Call>) -> Result<u64> {
    let fee = account()
        .await
        .execute(calls)
        .nonce(nonce().await)
        .estimate_fee()
        .await
        .context("Failed to estimate fee")
        .unwrap();

    Ok(fee.gas_consumed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tracing::info;

    // does not need proptest, as it doesn't use any input
    #[test]
    #[ignore] // needs a running katana
    fn bench_spawn() {
        let fee = execute(vec![("spawn", vec![])]).unwrap();

        info!(target: "bench_spawn", "Estimated fee: {fee}")
    }

    proptest! {
        #[test]
        #[ignore] // needs a running katana
        fn bench_move(c in "0x[0-4]") {
            let calls = vec![("spawn", vec![]), ("move", vec![FieldElement::from_hex_be(&c).unwrap()])];
            let fee = execute(calls).unwrap();

            info!(target: "bench_move", "Estimated fee: {fee},\tcalldata: {c}");
        }
    }
}
