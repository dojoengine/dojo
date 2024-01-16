use anyhow::Context;
use futures::future::join_all;
use katana_runner::BLOCK_TIME_IF_ENABLED;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::FieldElement;
use std::time::Duration;
use tokio::time::{sleep, Instant};

use crate::{parse_calls, BenchCall};

pub const ENOUGH_GAS: &str = "0x100000000000000000";
pub const BLOCK_TIME: Duration = Duration::from_millis(BLOCK_TIME_IF_ENABLED);
pub const N_TRANSACTIONS: usize = 2000;

#[katana_runner::katana_test]
async fn bench_katana_small() {
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

#[katana_runner::katana_test(1000, true, "../../target/release/katana")]
async fn bench_katana() {
    let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();
    let calldata_spawn = parse_calls(vec![BenchCall("spawn", vec![])]);
    let calldata_move =
        parse_calls(vec![BenchCall("move", vec![FieldElement::from_hex_be("0x3").unwrap()])]);

    // generating all needed accounts
    let accounts = runner.accounts();
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
    sleep(BLOCK_TIME).await;

    let transaction_hashes = join_all(move_txs.iter().map(|t| async {
        let r = t.send().await;
        (r, Instant::now())
    }))
    .await;

    // Unwraping and extracting the times
    let mut times = transaction_hashes
        .into_iter()
        .map(|r| {
            r.0.unwrap();
            r.1
        })
        .collect::<Vec<_>>();
    times.sort();

    // ⛏️ Block {block_number} mined with {tx_count} transactions

    let block_sizes = runner.block_sizes().await;
    let transaction_sum: u32 = block_sizes.iter().sum();

    assert_eq!(transaction_sum, 2 * runner.accounts_data().len() as u32);

    // time difference between first and last transaction
    println!("duration: {:?}", *times.last().unwrap() - *times.first().unwrap());
}
