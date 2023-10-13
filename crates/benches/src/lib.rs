#[cfg(test)]
mod tests {
    use clap_builder::Parser;
    use sozo::args::SozoArgs;
    use starknet_core::types::{FieldElement, TransactionReceipt};
    use starknet_providers::jsonrpc::JsonRpcResponse;

    const KATANA_ENDPOINT: &str = "http://localhost:5050";
    const WORLD: &str = "0x223b959926c92e10a5de78a76871fa40cefafbdce789137843df7c7b30e3e0";

    async fn paid_fee(tx: &str) -> FieldElement {
        let client = reqwest::Client::new();

        let res = client
            .post(KATANA_ENDPOINT)
            .body(format!(
                "{{\"jsonrpc\": \"2.0\",\"method\": \"starknet_getTransactionReceipt\",\"params\": [\"{}\"],\"id\": 1}}",
                tx,
            ))
            .header("Content-Type", "application/json")
            .send()
            .await
            .expect("Failed to send request");

        assert_eq!(res.status(), 200, "Couldn't fetch fee");

        let receipt: JsonRpcResponse<TransactionReceipt> =
            res.json().await.expect("Failed to parse response");

        let receipt = match match receipt {
            JsonRpcResponse::Success { result, .. } => result,
            JsonRpcResponse::Error { error, .. } => panic!("Katana parsing error: {:?}", error),
        } {
            TransactionReceipt::Invoke(receipt) => receipt,
            _ => panic!("Not an invoke transaction"),
        };

        receipt.actual_fee
    }

    #[tokio::test]
    async fn it_works() {
        let tx = "0x33d89d4a53a5d910abea61ec31554b21ce8d5f9ff2695ef712a85dcd98c1dda";
        let fee = paid_fee(tx).await;

        assert!(fee > FieldElement::ONE);
        // , "--world", WORLD
        let args = SozoArgs::parse_from(&["sozo", "execute"]);
        sozo::cli_main(args).expect("Execution error");
    }
}
