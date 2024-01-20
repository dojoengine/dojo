// Implementation of https://github.com/neotheprogramist/dojo/pull/16#discussion_r1453664539
use benches::spammer::spam_katana;
use benches::summary::BenchSummary;
use benches::BenchCall;
use katana_runner::KatanaRunner;
use starknet::core::types::FieldElement;

async fn run(runner: KatanaRunner, contract_address: FieldElement) -> BenchSummary {
    let spawn = BenchCall("spawn", vec![]);
    let calldata_move = BenchCall("move", vec![FieldElement::from_hex_be("0x3").unwrap()]);

    spam_katana(runner, contract_address, vec![spawn, calldata_move], 0, true).await
}

#[katana_runner::katana_test(1, true, "../../target/release/katana")]
async fn katana_benchmark_1() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(100, true, "../../target/release/katana")]
async fn katana_benchmark_100() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(200, true, "../../target/release/katana")]
async fn katana_benchmark_200() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(300, true, "../../target/release/katana")]
async fn katana_benchmark_300() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(400, true, "../../target/release/katana")]
async fn katana_benchmark_400() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(500, true, "../../target/release/katana")]
async fn katana_benchmark_500() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(750, true, "../../target/release/katana")]
async fn katana_benchmark_750() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(1000, true, "../../target/release/katana")]
async fn katana_benchmark_1000() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(1250, true, "../../target/release/katana")]
async fn katana_benchmark_1250() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(1500, true, "../../target/release/katana")]
async fn katana_benchmark_1500() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(1750, true, "../../target/release/katana")]
async fn katana_benchmark_1750() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}

#[katana_runner::katana_test(2000, true, "../../target/release/katana")]
async fn katana_benchmark_2000() {
    let results = run(runner, contract_address).await;
    results.dump().await;
}
