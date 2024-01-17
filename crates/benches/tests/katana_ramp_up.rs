// Implementation of https://github.com/neotheprogramist/dojo/pull/16#discussion_r1453664539
use futures::future::join_all;
use katana_runner::KatanaRunner;
use starknet::accounts::Account;
use starknet::core::types::FieldElement;
use std::time::Duration;
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    time::{sleep, Instant},
};

use benches::{parse_calls, BenchCall, ENOUGH_GAS};

#[derive(Debug)]
struct BenchResult {
    // All times are in miliseconds
    pub sending_time: u64,
    pub responses_span: u64,
    pub block_times: Vec<i64>,
    pub block_sizes: Vec<u32>,
    pub longest_confirmation_difference: u64,
    pub name: String,
}

impl BenchResult {
    pub fn relevant_blocks(&self) -> Vec<(u32, i64)> {
        let mut joined = self
            .block_sizes
            .iter()
            .zip(self.block_times.iter())
            .map(|(s, t)| (*s, *t))
            .collect::<Vec<_>>();

        while let Some((size, _time)) = joined.last() {
            if *size == 0 {
                joined.pop();
            } else {
                break;
            }
        }

        let mut start = 0;
        for (i, (size, _time)) in joined.iter().enumerate().rev() {
            if *size == 0 {
                start = i + 1;
                break;
            }
        }

        joined.drain(start..).collect()
    }

    pub async fn dump(&self) {
        let mut file =
            OpenOptions::new().create(true).append(true).open("bench_results.txt").await.unwrap();

        let content = format!("{}:\n{}\n\n", self.name, self);
        file.write_all(content.as_bytes()).await.unwrap();
    }
}

impl std::fmt::Display for BenchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "sending time: {}", self.sending_time)?;
        writeln!(f, "responses span: {}", self.responses_span)?;
        writeln!(f, "longest confirmation difference: {}", self.longest_confirmation_difference)?;
        writeln!(f, "block times: {:?}", self.block_times)?;
        writeln!(f, "block sizes: {:?}", self.block_sizes)?;
        writeln!(f, "relevant blocks: {:?}", self.relevant_blocks())?;
        Ok(())
    }
}

async fn run(runner: KatanaRunner) -> BenchResult {
    let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();
    let calldata_spawn = parse_calls(vec![BenchCall("spawn", vec![])]);
    let calldata_move =
        parse_calls(vec![BenchCall("move", vec![FieldElement::from_hex_be("0x3").unwrap()])]);

    // generating all needed accounts
    let accounts = runner.accounts();
    let wait_time = Duration::from_millis(accounts.len() as u64 * 40);
    let (spawn_txs, move_txs): (Vec<_>, Vec<_>) = accounts.iter()
            .map(|account| {
                let spawn_call =
                    account.execute(calldata_spawn.clone()).nonce(FieldElement::ONE).max_fee(max_fee);
                let move_call = account
                    .execute(calldata_move.clone())
                    .nonce(FieldElement::TWO)
                    .max_fee(max_fee);
                (spawn_call, move_call)
            })
            // .collect::<Vec<_>>();
            .unzip();

    // running a spawn for each account
    join_all(spawn_txs.iter().map(|t| t.send())).await;
    sleep(wait_time).await;
    runner.block_sizes().await;

    let before = Instant::now();
    let transaction_hashes = join_all(move_txs.iter().map(|t| async {
        let r = t.send().await;
        (r, Instant::now())
    }))
    .await;
    let sending_time = before.elapsed().as_millis() as u64;
    sleep(wait_time).await;

    // Unwraping and extracting the times
    let mut times = transaction_hashes
        .into_iter()
        .map(|r| {
            r.0.unwrap();
            r.1
        })
        .collect::<Vec<_>>();
    times.sort();
    let durations = times.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();

    let longest_confirmation_difference =
        (durations.last().unwrap().as_millis() - durations.first().unwrap().as_millis()) as u64;

    let block_sizes = runner.block_sizes().await;
    let transaction_sum: u32 = block_sizes.iter().sum();

    assert_eq!(transaction_sum, 2 * runner.accounts_data().len() as u32);

    // time difference between first and last transaction
    let block_times = runner.block_times().await;
    let block_sizes = runner.block_sizes().await;
    let name = format!("benchmark {} transactions", runner.accounts_data().len());
    let responses_span = (*times.last().unwrap() - *times.first().unwrap()).as_millis() as u64;
    BenchResult {
        sending_time,
        responses_span,
        block_times,
        block_sizes,
        longest_confirmation_difference,
        name,
    }
}

#[katana_runner::katana_test(100, true, "../../target/release/katana")]
async fn katana_benchmark_100() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(200, true, "../../target/release/katana")]
async fn katana_benchmark_200() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(300, true, "../../target/release/katana")]
async fn katana_benchmark_300() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(400, true, "../../target/release/katana")]
async fn katana_benchmark_400() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(500, true, "../../target/release/katana")]
async fn katana_benchmark_500() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(750, true, "../../target/release/katana")]
async fn katana_benchmark_750() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(1000, true, "../../target/release/katana")]
async fn katana_benchmark_1000() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(1250, true, "../../target/release/katana")]
async fn katana_benchmark_1250() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(1500, true, "../../target/release/katana")]
async fn katana_benchmark_1500() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(1750, true, "../../target/release/katana")]
async fn katana_benchmark_1750() {
    let results = run(runner).await;
    results.dump().await;
}

#[katana_runner::katana_test(2000, true, "../../target/release/katana")]
async fn katana_benchmark_2000() {
    let results = run(runner).await;
    results.dump().await;
}
