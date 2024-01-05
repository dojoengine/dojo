mod helpers;

use anyhow::Result;
use futures::executor::block_on;
use futures::future;
pub use helpers::log;
use helpers::*;
use lazy_static::lazy_static;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;
use tokio::runtime::Runtime;

type OwnerAccount = SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>;

const KATANA_ENDPOINT: &str = "http://localhost:5050";
const CONTRACT_ADDRESS: &str = "0x297bde19ca499fd8a39dd9bedbcd881a47f7b8f66c19478ce97d7de89e6112e";

const ACCOUNT_ADDRESS: &str = "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973";
const PRIVATE_KEY: &str = "0x1800000000300000180000000000030000000000003006001800006600";

pub struct BenchCall(pub &'static str, pub Vec<FieldElement>);

lazy_static! {
    static ref CONTRACT: FieldElement = FieldElement::from_hex_be(CONTRACT_ADDRESS).unwrap();
    pub static ref RUNTIME: Runtime = Runtime::new().unwrap();
}

pub fn estimate_gas_last(calls: Vec<BenchCall>) -> Result<u64> {
    let mut calls = parse_calls(calls);
    let all = calls.clone();
    calls.pop().expect("Empty calls vector"); // remove last call

    let _rt = RUNTIME.enter();
    block_on(async move {
        let (whole_gas, before_gas) =
            future::try_join(estimate_calls(all), estimate_calls(calls)).await?;
        Ok(whole_gas - before_gas)
    })
}

pub fn estimate_gas(call: BenchCall) -> Result<u64> {
    let calls = parse_calls(vec![call]);
    let _rt = RUNTIME.enter();
    block_on(async move { estimate_calls(calls).await })
}

pub fn estimate_gas_multiple(calls: Vec<BenchCall>) -> Result<u64> {
    let calls = parse_calls(calls);
    let _rt = RUNTIME.enter();
    block_on(async move { estimate_calls(calls).await })
}

pub async fn estimate_gas_async(calls: Vec<BenchCall>) -> Result<u64> {
    let calls = parse_calls(calls);
    estimate_calls(calls).await
}

#[cfg(test)]
mod tests {

    use futures::future::join_all;
    use proptest::prelude::*;

    use super::*;

    // does not need proptest, as it doesn't use any input
    #[test]
    #[ignore] // needs a running katana
    fn bench_default_spawn() {
        let fee = estimate_gas(BenchCall("spawn", vec![])).unwrap();

        log("bench_spawn", fee, "");
    }

    #[tokio::test]
    #[ignore] // needs a running katana
    async fn bench_katana() {
        let args = vec![FieldElement::from_hex_be("0x1").unwrap()];

        let nonce = nonce().await;
        execute_calls(
            parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]),
            nonce,
        )
        .await
        .unwrap();

        let calls = (0..1).map(move |i: u64| {
            execute_calls(parse_calls(vec![BenchCall("move", args.clone())]), nonce + i.into())
        });

        join_all(calls).await.into_iter().for_each(|r| r.unwrap());
    }

    proptest! {
        #[test]
        #[ignore] // needs a running katana
        fn bench_default_move(c in "0x[0-4]") {
            let fee = estimate_gas_last(vec![
                BenchCall("spawn", vec![]),
                BenchCall("move", vec![FieldElement::from_hex_be(&c).unwrap()])
            ]).unwrap();

            log("bench_move", fee, &c);
        }

        #[test]
        #[ignore] // needs a running katana
        fn bench_default_spawn_and_move(c in "0x[0-4]") {
            let fee = estimate_gas_multiple(vec![
                BenchCall("spawn", vec![]),
                BenchCall("move", vec![FieldElement::from_hex_be(&c).unwrap()])
            ]).unwrap();

            log("bench_spawn_move", fee, &c);
        }
    }
}
