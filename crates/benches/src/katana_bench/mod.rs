#[cfg(test)]
mod tests {
    use starknet::core::types::FieldElement;

    use crate::*;

    #[tokio::test]
    #[ignore] // needs a running katana
    async fn bench_katana() {
        let args = vec![FieldElement::from_hex_be("0x1").unwrap()];

        let nonce = cached_nonce().await;

        execute_calls(
            parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]),
            nonce,
        )
        .await
        .unwrap();

        execute_calls(
            parse_calls(vec![BenchCall("spawn", vec![]), BenchCall("move", args.clone())]),
            nonce + FieldElement::ONE,
        )
        .await
        .unwrap();

        // let calls = (1..3).map(move |i: u64| {
        //     execute_calls(parse_calls(vec![BenchCall("move", args.clone())]), nonce + i.into())
        // });

        // let transaction_hashes = join_all(calls).await.into_iter().map(|r| r.unwrap());
    }
}
