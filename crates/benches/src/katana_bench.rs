use anyhow::Context;
use futures::future::join_all;
use katana_runner::BLOCK_TIME_IF_ENABLED;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::FieldElement;
use std::time::Duration;
use tokio::time::{sleep, Instant};

use crate::{parse_calls, BenchCall, ENOUGH_GAS};

pub const BLOCK_TIME: Duration = Duration::from_millis(BLOCK_TIME_IF_ENABLED);
pub const N_TRANSACTIONS: usize = 2000; // depends on https://github.com/neotheprogramist/dojo/blob/6dbd719c09c01189f0b51f7381830fe451f268aa/crates/katana/core/src/pool.rs#L33-L34

#[katana_runner::katana_test]
async fn bench_katana_small() {
    let args = vec![FieldElement::from_hex_be("0x1").unwrap()];
    let prefunded = runner.account(0);
    runner.deploy("contracts/Scarb.toml", "contracts/scripts/auth.sh").await.unwrap();

    prefunded
        .execute(parse_calls(
            vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())],
            &contract_address,
        ))
        .nonce(prefunded.get_nonce().await.unwrap())
        .send()
        .await
        .context("Failed to execute")
        .unwrap();
}

#[katana_runner::katana_test(2000, true, "../../target/release/katana")]
async fn bench_katana() {
    let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();
    let calldata_spawn = parse_calls(vec![BenchCall("spawn", vec![])], &contract_address);
    let calldata_move = parse_calls(
        vec![BenchCall("move", vec![FieldElement::from_hex_be("0x3").unwrap()])],
        &contract_address,
    );

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
    sleep(Duration::from_secs(100)).await;
    runner.block_sizes().await;

    let before = Instant::now();
    let transaction_hashes = join_all(move_txs.iter().map(|t| async {
        let r = t.send().await;
        (r, Instant::now())
    }))
    .await;
    println!("sending: {}", before.elapsed().as_millis());
    sleep(Duration::from_secs(200)).await;

    // Unwraping and extracting the times
    let mut times = transaction_hashes
        .into_iter()
        .map(|r| {
            r.0.unwrap();
            r.1
        })
        .collect::<Vec<_>>();
    let durations = times.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();

    times.sort();

    println!("min sending: {}", durations.first().unwrap().as_millis());
    println!("max sending: {}", durations.last().unwrap().as_millis());

    let block_sizes = runner.block_sizes().await;
    let transaction_sum: u32 = block_sizes.iter().sum();

    dbg!(runner.block_times().await);
    dbg!(block_sizes);

    assert_eq!(transaction_sum, 2 * runner.accounts_data().len() as u32);

    // time difference between first and last transaction
    println!("duration: {:?}", *times.last().unwrap() - *times.first().unwrap());
}
