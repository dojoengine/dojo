use futures::future::join_all;
use katana_runner::KatanaRunner;
use starknet::accounts::Account;
use starknet::core::types::FieldElement;
use std::time::Duration;
use tokio::time::{sleep, Instant};

use crate::summary::BenchSummary;
use crate::{parse_calls, BenchCall, ENOUGH_GAS};

pub async fn spam_katana(
    runner: KatanaRunner,
    contract_address: FieldElement,
    mut calldata: Vec<BenchCall>,
) -> BenchSummary {
    let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();

    let transaction_sum_before: u32 = runner.block_sizes().await.iter().sum();

    let final_call = parse_calls(vec![calldata.pop().unwrap()], &contract_address);

    // generating all needed accounts
    let mut nonce = FieldElement::ONE;
    let accounts = runner.accounts();
    let wait_time = Duration::from_millis(accounts.len() as u64 * 40);

    for call in parse_calls(calldata, &contract_address) {
        let transactions = accounts
            .iter()
            .map(|account| account.execute(vec![call.clone()]).nonce(nonce).max_fee(max_fee))
            .collect::<Vec<_>>();

        join_all(transactions.iter().map(|t| t.send())).await;

        sleep(wait_time).await;
        runner.blocks_until_empty().await;
        nonce += FieldElement::ONE;
    }

    let move_txs = accounts
        .iter()
        .map(|account| {
            let move_call = account.execute(final_call.clone()).nonce(nonce).max_fee(max_fee);
            move_call
        })
        .collect::<Vec<_>>();

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
    let _transaction_sum = block_sizes.iter().sum::<u32>() - transaction_sum_before;

    // assert_eq!(transaction_sum, 2 * accounts.len() as u32);

    // time difference between first and last transaction
    let block_times = runner.block_times().await;
    let block_sizes = runner.block_sizes().await;
    let name = format!("benchmark {} transactions", accounts.len());
    let responses_span = (*times.last().unwrap() - *times.first().unwrap()).as_millis() as u64;
    BenchSummary {
        sending_time,
        responses_span,
        block_times,
        block_sizes,
        longest_confirmation_difference,
        name,
        stats: None,
    }
}
