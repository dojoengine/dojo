#[cfg(test)]
mod timings_stats;

use std::time::Duration;

pub const ENOUGH_GAS: &str = "0x100000000000000000";
pub const BLOCK_TIME: Duration = Duration::from_secs(3);
pub const N_TRANSACTIONS: usize = 2000;

#[cfg(test)]
mod tests {

    use futures::future::join_all;
    use starknet::accounts::Account;
    use starknet::core::types::FieldElement;
    use tokio::time::{sleep, Instant};

    use super::*;
    use crate::katana_bench::timings_stats::timetable_stats;
    use crate::*;

    // #[katana_runner::katana_test(10000, true, "../../target/release/katana")]
    // async fn bench_katana() {
    //     let account_manager = account_manager().await;
    //     let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();
    //     let calldata_spawn = parse_calls(vec![BenchCall("spawn", vec![])]);
    //     let calldata_move =
    //         parse_calls(vec![BenchCall("move", vec![FieldElement::from_hex_be("0x3").unwrap()])]);

    //     // generating all needed accounds
    //     let accounts = join_all((0..N_TRANSACTIONS).map(|_| account_manager.next())).await;
    //     let (spawn_txs, move_txs): (Vec<_>, Vec<_>) = accounts
    //         .iter()
    //         .map(|(account, nonce)| {
    //             let spawn_call =
    //                 account.execute(calldata_spawn.clone()).nonce(*nonce).max_fee(max_fee);
    //             let move_call = account
    //                 .execute(calldata_move.clone())
    //                 .nonce(*nonce + 1u8.into())
    //                 .max_fee(max_fee);
    //             (spawn_call, move_call)
    //         })
    //         // .collect::<Vec<_>>();
    //         .unzip();

    //     // running a spawn for each account
    //     join_all(spawn_txs.iter().map(|t| t.send())).await;
    //     sleep(BLOCK_TIME).await;

    //     let transaction_hashes = join_all(move_txs.iter().map(|t| async {
    //         let r = t.send().await;
    //         (r, Instant::now())
    //     }))
    //     .await;

    //     // Unwraping and extracting the times
    //     let mut times = transaction_hashes
    //         .into_iter()
    //         .map(|r| {
    //             r.0.unwrap();
    //             r.1
    //         })
    //         .collect::<Vec<_>>();
    //     times.sort();

    //     // time difference between first and last transaction
    //     println!("duration: {:?}", *times.last().unwrap() - *times.first().unwrap());

    //     // printing some minimal stats
    //     let max = timetable_stats(times);
    //     assert!(max > 500);
    // }
}
