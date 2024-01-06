#[cfg(test)]
mod tests {
    use std::time::Instant;

    use futures::future::join_all;
    use starknet::{accounts::Account, core::types::FieldElement};

    use crate::*;

    #[tokio::test]
    #[ignore] // needs a running katana
    async fn bench_katana() {
        let args = vec![FieldElement::from_hex_be("0x1").unwrap()];
        let account_manager = account_manager().await;

        let calls = parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]);
        let max_fee = FieldElement::from_hex_be("0x100000000000000000").unwrap();

        let accounts = join_all((0..1000u32).into_iter().map(|_| account_manager.next())).await;
        let transactions = accounts
            .iter()
            .map(|(account, nonce)| account.execute(calls.clone()).nonce(*nonce).max_fee(max_fee))
            .collect::<Vec<_>>();

        let before = Instant::now();
        let transaction_hashes = join_all(transactions.iter().map(|t| t.send())).await;
        println!("duration: {:?}", Instant::now() - before);

        transaction_hashes.into_iter().for_each(|r| {
            r.unwrap();
        });
    }
}
