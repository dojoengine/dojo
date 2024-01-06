#[cfg(test)]
mod tests {
    use anyhow::Context;
    use starknet::{
        accounts::{Account, ConnectedAccount},
        core::types::FieldElement,
    };

    use crate::*;

    #[tokio::test]
    #[ignore] // needs a running katana
    async fn bench_katana() {
        let args = vec![FieldElement::from_hex_be("0x1").unwrap()];
        let account_manager = account_manager().await;

        for i in 0..50u32 {
            let account = account_manager.next().await;

            let nonce = account.get_nonce().await.unwrap();
            let calls =
                parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]);

            account.execute(calls).nonce(nonce).send().await.context("Failed to execute").unwrap();
        }

        // for account in account_manager().await {
        //     let calls =
        //         parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]);

        //     account
        //         .execute(calls)
        //         .nonce(account.get_nonce().await.unwrap())
        //         .send()
        //         .await
        //         .context("Failed to execute")
        //         .unwrap();
        // }

        // let nonce = cached_nonce().await;
        // execute_calls(
        //     parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]),
        //     nonce,
        // )
        // .await
        // .unwrap();

        // execute_calls(
        //     parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]),
        //     nonce + FieldElement::ONE,
        // )
        // .await
        // .unwrap();

        // let calls = (1..3).map(move |i: u64| {
        //     execute_calls(parse_calls(vec![BenchCall("move", args.clone())]), nonce + i.into())
        // });

        // let transaction_hashes = join_all(calls).await.into_iter().map(|r| r.unwrap());
    }
}
