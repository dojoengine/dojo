#[cfg(test)]
mod tests {
    use std::process::Command;
    // use clap_builder::Parser;
    use starknet::core::types::{FieldElement, TransactionReceipt};
    use starknet::providers::jsonrpc::JsonRpcResponse;

    const KATANA_ENDPOINT: &str = "http://localhost:5050";
    const WORLD: &str = "0x223b959926c92e10a5de78a76871fa40cefafbdce789137843df7c7b30e3e0";

    fn paid_fee(tx: &str) -> FieldElement {
        let client = reqwest::blocking::Client::new();

        let res = client
            .post(KATANA_ENDPOINT)
            .body(format!(
                "{{\"jsonrpc\": \"2.0\",\"method\": \"starknet_getTransactionReceipt\",\"params\": [\"{}\"],\"id\": 1}}",
                tx,
            ))
            .header("Content-Type", "application/json")
            .send()
            .expect("Failed to send request");

        assert_eq!(res.status(), 200, "Couldn't fetch fee");

        let receipt: JsonRpcResponse<TransactionReceipt> =
            res.json().expect("Failed to parse response");

        let receipt = match match receipt {
            JsonRpcResponse::Success { result, .. } => result,
            JsonRpcResponse::Error { error, .. } => panic!("Katana parsing error: {:?}", error),
        } {
            TransactionReceipt::Invoke(receipt) => receipt,
            _ => panic!("Not an invoke transaction"),
        };

        receipt.actual_fee
    }

    fn execute(entrypoint: &str, calldata: Option<String>) -> String {
        let mut args = vec![
            "execute",
            "--account-address",
            "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
            "--private-key",
            "0x1800000000300000180000000000030000000000003006001800006600",
            "--rpc-url",
            "http://localhost:5050",
            "--world",
            WORLD,
            entrypoint,
        ];
        if let Some(ref calldata) = calldata {
            args.extend(["--calldata", calldata]);
        }

        // looks like it doesn't work at the moment, so using installed version of sozo
        // sozo::cli_main(SozoArgs::parse_from(args)).expect("Execution error");

        let output = Command::new("sozo").args(args).output().expect("failed to execute process");
        assert!(
            output.status.success(),
            "Execution failed at: {}",
            String::from_utf8(output.stderr).unwrap()
        );
        let tx = String::from_utf8(output.stdout)
            .expect("Failed to parse output")
            .strip_prefix("Transaction: ")
            .expect("Invalid output")
            .trim()
            .to_owned();
        assert_eq!(&tx[0..2], "0x", "Invalid tx hash");
        tx
    }

    #[test]
    fn basic_contract_call() {
        let tx = execute("spawn", None);

        let fee = paid_fee(&tx);
        assert!(fee > FieldElement::ONE);
        println!("Tx: {}, fee: {}", tx, fee);
    }
}
