pub mod account_manager;
pub mod helpers;
pub mod katana_bench;

pub use account_manager::*;
use anyhow::Result;
use futures::executor::block_on;
use futures::future;
pub use helpers::*;
use lazy_static::lazy_static;
use starknet::core::types::FieldElement;
use tokio::runtime::Runtime;

const KATANA_ENDPOINT: &str = "http://localhost:5050";
const CONTRACT_ADDRESS: &str = "0x38979e719956897617c83fcbc69de9bc56491fd10c093dd8492b92ee7326d98";

const ACCOUNT_ADDRESS: &str = "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973";
const _PRIVATE_KEY: &str = "0x1800000000300000180000000000030000000000003006001800006600";

pub struct BenchCall(pub &'static str, pub Vec<FieldElement>);

lazy_static! {
    static ref CONTRACT: FieldElement = FieldElement::from_hex_be(CONTRACT_ADDRESS).unwrap();
    pub static ref RUNTIME: Runtime = Runtime::new().unwrap();
}

pub fn estimate_gas_last(account: &OwnerAccount, calls: Vec<BenchCall>) -> Result<u64> {
    let mut calls = parse_calls(calls);
    let all = calls.clone();
    calls.pop().expect("Empty calls vector"); // remove last call

    let _rt = RUNTIME.enter();
    block_on(async move {
        let (whole_gas, before_gas) =
            future::try_join(estimate_calls(&account, all), estimate_calls(&account, calls))
                .await?;
        Ok(whole_gas - before_gas)
    })
}

pub fn estimate_gas(account: &OwnerAccount, call: BenchCall) -> Result<u64> {
    let calls = parse_calls(vec![call]);
    let _rt = RUNTIME.enter();
    block_on(async move { estimate_calls(account, calls).await })
}

pub fn estimate_gas_multiple(account: &OwnerAccount, calls: Vec<BenchCall>) -> Result<u64> {
    let calls = parse_calls(calls);
    let _rt = RUNTIME.enter();
    block_on(async move { estimate_calls(account, calls).await })
}

pub async fn estimate_gas_async(account: &OwnerAccount, calls: Vec<BenchCall>) -> Result<u64> {
    let calls = parse_calls(calls);
    estimate_calls(account, calls).await
}

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use helpers::log;
    use katana_runner::runner;
    use proptest::prelude::*;
    use starknet::accounts::{Account, ConnectedAccount};

    use super::*;

    // does not need proptest, as it doesn't use any input
    #[katana_runner::katana_test]
    async fn bench_default_spawn() {
        runner.deploy("contracts/Scarb.toml", "contracts/scripts/auth.sh").await.unwrap();

        let fee = estimate_gas(&runner.account(0), BenchCall("spawn", vec![])).unwrap();

        log("bench_spawn", fee, "");
    }

    #[katana_runner::katana_test]
    async fn bench_katana() {
        let args = vec![FieldElement::from_hex_be("0x1").unwrap()];
        let prefunded = runner.account(0);
        runner.deploy("contracts/Scarb.toml", "contracts/scripts/auth.sh").await.unwrap();

        prefunded
            .execute(parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]))
            .nonce(prefunded.get_nonce().await.unwrap())
            .send()
            .await
            .context("Failed to execute")
            .unwrap();
    }

    proptest! {
        #[test]
        fn bench_default_move(c in "0x[0-4]") {
            runner!(bench_default_move);

            let fee = estimate_gas_last(&runner.account(0), vec![
                BenchCall("spawn", vec![]),
                BenchCall("move", vec![FieldElement::from_hex_be(&c).unwrap()])
            ]).unwrap();

            log("bench_move", fee, &c);
        }

        #[test]
        fn bench_default_spawn_and_move(c in "0x[0-4]") {
            runner!(bench_default_spawn_and_move);

            let fee = estimate_gas_multiple(&runner.account(0), vec![
                BenchCall("spawn", vec![]),
                BenchCall("move", vec![FieldElement::from_hex_be(&c).unwrap()])
            ]).unwrap();

            log("bench_spawn_move", fee, &c);
        }
    }
}
