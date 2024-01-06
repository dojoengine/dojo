#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use futures::future::join_all;
    use starknet::{accounts::Account, core::types::FieldElement};

    use crate::*;

    const ENOUGH_GAS: &str = "0x100000000000000000";

    #[tokio::test]
    #[ignore] // needs a running katana
    async fn bench_katana() {
        send_spawn().await;
        let account_manager = account_manager().await;
        let max_fee = FieldElement::from_hex_be(ENOUGH_GAS).unwrap();
        let calldata = parse_calls(vec![BenchCall(
            "move",
            vec![FieldElement::from_hex_be("0x1").unwrap()].clone(),
        )]);

        let accounts = join_all((0..500u32).into_iter().map(|_| account_manager.next())).await;
        let transactions = accounts
            .iter()
            .map(|(account, nonce)| {
                account.execute(calldata.clone()).nonce(*nonce).max_fee(max_fee)
            })
            .collect::<Vec<_>>();

        let transaction_hashes = join_all(transactions.iter().map(|t| async {
            let r = t.send().await;
            (r, Instant::now())
        }))
        .await;

        let mut times = transaction_hashes
            .into_iter()
            .map(|r| {
                r.0.unwrap();
                r.1
            })
            .collect::<Vec<_>>();
        times.sort();

        println!("duration: {:?}", *times.last().unwrap() - *times.first().unwrap());

        timetable_stats(times);
    }

    async fn send_spawn() {
        let account_manager = account_manager().await;

        let (account, nonce) = account_manager.next().await;
        account
            .execute(parse_calls(vec![BenchCall("spawn", vec![])]))
            .nonce(nonce)
            .max_fee(FieldElement::from_hex_be(ENOUGH_GAS).unwrap())
            .send()
            .await
            .unwrap();
    }

    fn timetable_stats(times: Vec<Instant>) {
        let time_window = Duration::from_secs(3);
        let mut left = 0;
        let mut right = 0;
        let mut initial_phase = true;
        let mut max = 0;
        let mut min_after_initial = 0;

        loop {
            if right == times.len() {
                break;
            }

            if times[right] - times[left] > time_window {
                left += 1;
                initial_phase = false;
            } else {
                right += 1;
                let current = right - left;
                if current > max {
                    max = current;
                }

                if !initial_phase {
                    if current < min_after_initial {
                        min_after_initial = current;
                    }
                }
            }
        }

        println!("max: {}", max);
        println!("min_after_initial: {}", min_after_initial);
    }
}
