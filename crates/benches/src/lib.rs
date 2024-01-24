pub mod helpers;
pub mod spammer;
pub mod summary;

use anyhow::Result;
use futures::executor::block_on;
use futures::future;
pub use helpers::*;
pub use katana_runner::runner;
use lazy_static::lazy_static;
pub use starknet::core::types::FieldElement;
use tokio::runtime::Runtime;

pub const ENOUGH_GAS: &str = "0x100000000000000000";

lazy_static! {
    pub static ref RUNTIME: Runtime = Runtime::new().unwrap();
}

pub fn estimate_gas_last(
    account: &OwnerAccount,
    calls: Vec<BenchCall>,
    contract: &FieldElement,
) -> Result<u64> {
    let mut calls = parse_calls(calls, contract);
    let all = calls.clone();
    calls.pop().expect("Empty calls vector"); // remove last call

    let _rt = RUNTIME.enter();
    block_on(async move {
        let (whole_gas, before_gas) =
            future::try_join(estimate_calls(account, all), estimate_calls(account, calls)).await?;
        Ok(whole_gas - before_gas)
    })
}

pub fn estimate_gas(
    account: &OwnerAccount,
    call: BenchCall,
    contract: &FieldElement,
) -> Result<u64> {
    let calls = parse_calls(vec![call], contract);
    let _rt = RUNTIME.enter();
    block_on(async move { estimate_calls(account, calls).await })
}

pub fn estimate_gas_multiple(
    account: &OwnerAccount,
    calls: Vec<BenchCall>,
    contract: &FieldElement,
) -> Result<u64> {
    let calls = parse_calls(calls, contract);
    let _rt = RUNTIME.enter();
    block_on(async move { estimate_calls(account, calls).await })
}

pub async fn estimate_gas_async(
    account: &OwnerAccount,
    calls: Vec<BenchCall>,
    contract: &FieldElement,
) -> Result<u64> {
    let calls = parse_calls(calls, contract);
    estimate_calls(account, calls).await
}

#[cfg(not(feature = "skip-gas-benchmarks"))]
#[cfg(test)]
mod tests {
    use helpers::log;
    use katana_runner::runner;
    use proptest::prelude::*;
    use starknet::core::types::FieldElement;

    use super::*;

    // does not need proptest, as it doesn't use any input
    #[katana_runner::katana_test]
    async fn bench_default_spawn() {
        let fee = estimate_gas(&runner.account(0), BenchCall("spawn", vec![]), &contract_address)
            .unwrap();

        log("bench_spawn", fee, "");
    }

    proptest! {
        #[test]
        fn bench_default_move(c in "0x[0-4]") {
            runner!(bench_default_move);

            let fee = estimate_gas_last(&runner.account(0), vec![
                BenchCall("spawn", vec![]),
                // BenchCall("move", vec![FieldElement::from_hex_be(&c).unwrap()])
            ], contract_address).unwrap();

            log("bench_move", fee, &c);
        }

        #[test]
        fn bench_default_spawn_and_move(c in "0x[0-4]") {
            runner!(bench_default_spawn_and_move);

            let fee = estimate_gas_multiple(&runner.account(0), vec![
                BenchCall("spawn", vec![]),
                BenchCall("move", vec![FieldElement::from_hex_be(&c).unwrap()])
            ], contract_address).unwrap();

            log("bench_spawn_move", fee, &c);
        }
    }
}
