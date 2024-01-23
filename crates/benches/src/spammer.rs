use std::time::Duration;

use futures::future::join_all;
use katana_runner::KatanaRunner;
use starknet::accounts::{Account, SingleOwnerAccount};
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;
use tokio::time::{sleep, Instant};

use crate::summary::BenchSummary;
use crate::{parse_calls, BenchCall, ENOUGH_GAS};

async fn spam_no_stats(
    runner: &KatanaRunner,
    accounts: &[SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>],
    contract_address: &FieldElement,
    calldata: Vec<BenchCall>,
    wait_time: Duration,
) -> FieldElement {
    let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();
    let mut nonce = FieldElement::ONE;

    for call in parse_calls(calldata, contract_address) {
        let transactions = accounts
            .iter()
            .map(|account| account.execute(vec![call.clone()]).nonce(nonce).max_fee(max_fee))
            .collect::<Vec<_>>();

        join_all(transactions.iter().map(|t| t.send())).await;

        sleep(wait_time).await;
        runner.blocks_until_empty().await;
        nonce += FieldElement::ONE;
    }

    nonce
}

pub async fn spam_katana(
    runner: KatanaRunner,
    contract_address: FieldElement,
    mut calldata: Vec<BenchCall>,
    additional_sleep: u64,
    sequential: bool,
) -> BenchSummary {
    let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();

    let transaction_sum_before: u32 = runner.block_sizes().await.iter().sum();
    let steps_before = runner.steps().await;

    // generating all needed accounts
    let accounts = runner.accounts();
    let wait_time = Duration::from_millis(accounts.len() as u64 * 35 + 3000 + additional_sleep);
    let name = format!(
        "Benchmark: {} accounts, {} transactions, {} calls",
        accounts.len(),
        calldata.last().unwrap().0,
        calldata.len()
    );

    let calls = match sequential {
        true => {
            let calls = parse_calls(calldata, &contract_address);
            calldata = vec![];
            calls
        }
        false => parse_calls(vec![calldata.pop().unwrap()], &contract_address),
    };

    let expected_transactions = (calldata.len() + 1) * accounts.len();

    // transactions preparing for the benchmarked one
    let nonce = spam_no_stats(&runner, &accounts, &contract_address, calldata, wait_time).await;

    // the benchmarked transaction
    let final_transactions = accounts
        .iter()
        .map(|account| {
            let move_call = account.execute(calls.clone()).nonce(nonce).max_fee(max_fee);
            move_call
        })
        .collect::<Vec<_>>();

    let before = Instant::now();
    let transaction_hashes = join_all(final_transactions.iter().map(|t| async {
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
    let mut durations = times.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
    durations.sort();

    let longest_confirmation_difference = match durations.len() {
        0 => 0,
        _ => durations.last().unwrap().as_millis() - durations.first().unwrap().as_millis(),
    } as u64;

    let block_sizes = runner.block_sizes().await;
    let transaction_sum = block_sizes.iter().sum::<u32>() - transaction_sum_before;
    assert_eq!(transaction_sum as usize, expected_transactions);

    // time difference between first and last transaction
    let block_times = runner.block_times().await;
    let block_sizes = runner.block_sizes().await;
    let responses_span = (*times.last().unwrap() - *times.first().unwrap()).as_millis() as u64;

    let mut steps = runner.steps().await;
    steps.drain(0..steps_before.len());

    // Check if transactions actually passed as well
    assert_eq!(steps.len(), expected_transactions);

    BenchSummary {
        sending_time,
        responses_span,
        block_times,
        block_sizes,
        longest_confirmation_difference,
        name,
        stats: None,
        steps: steps.into_iter().sum(),
    }
}
