#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Context, Result};
    use hex::ToHex;
    use lazy_static::lazy_static;
    use reqwest::Url;
    use starknet::accounts::{Call, Execution, ExecutionEncoding, SingleOwnerAccount};
    use starknet::core::types::{BlockId, BlockTag, FieldElement, TransactionReceipt};
    use starknet::core::utils::get_selector_from_name;
    use starknet::providers::{
        jsonrpc::{HttpTransport, JsonRpcResponse},
        JsonRpcClient, Provider,
    };
    use starknet::signers::{LocalWallet, SigningKey};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use proptest::prelude::*;

    const KATANA_ENDPOINT: &str = "http://localhost:5050";
    const WORLD: &str = "0x223b959926c92e10a5de78a76871fa40cefafbdce789137843df7c7b30e3e0";
    const EXECUTABLE: &str = "sozo"; // using system installed version of sozo

    #[derive(Clone, Debug)]
    struct Keypair(String, String);

    #[derive(Clone, Debug)]
    struct TransactionSequence(String, Keypair);

    fn execute(entrypoint: &str, calldata: Option<String>) -> Result<TransactionSequence> {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let result = COUNTER.fetch_add(1, Ordering::Relaxed);

        let keypair = KEY_PAIRS[result % KEY_PAIRS.len()].clone();

        TransactionSequence::execute_on(keypair, entrypoint, calldata)
    }

    impl TransactionSequence {
        fn then(self, entrypoint: &str, calldata: Option<String>) -> Result<TransactionSequence> {
            Self::execute_on(self.1, entrypoint, calldata)
        }

        fn tx(self) -> String {
            self.0
        }

        fn execute_on(
            keypair: Keypair,
            entrypoint: &str,
            calldata: Option<String>,
        ) -> Result<TransactionSequence> {
            let Keypair(address, private) = &keypair;

            let mut args = vec![
                "execute",
                "--account-address",
                &address,
                "--private-key",
                &private,
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

            let output = Command::new(EXECUTABLE)
                .args(args)
                .output()
                .context("failed to execute process")?;

            if !output.status.success() {
                return Err(anyhow!("Execution failed"));
            }

            let tx = String::from_utf8(output.stdout)
                .context("Failed to parse output")?
                .strip_prefix("Transaction: ")
                .context("Invalid output")?
                .trim()
                .to_owned();

            if &tx[0..2] != "0x" {
                return Err(anyhow!("Invalid tx hash"));
            }

            Ok(TransactionSequence(tx, keypair))
        }
    }

    lazy_static! {
        // load output from katana launched with `katana --accounts 255 > prefunded.txt`
        // this allows for up to 255 concurrent accounts without the need to fund them
        static ref KEY_PAIRS: Vec<Keypair> = {
            // load from file
            let file_contents =
                std::fs::read_to_string("prefunded.txt").expect("Failed to read prefunded.txt");

            // parse just the hexadecimal values
            let hexes = file_contents
                .lines()
                .filter(|l| l.contains('|'))
                .map(|l| l.split('|').skip(2).next().unwrap().trim().to_owned())
                .collect::<Vec<_>>();

            // convert to pairs of address and private key
            hexes.chunks(3)
                .map(|chunk| Keypair(chunk[0].to_owned(), chunk[1].to_owned())) // Address, private key
                .collect::<Vec<_>>()
        };

        static ref CONTRACT: FieldElement = FieldElement::from_hex_be(
            "0x5d69ccf0644b87204e143d2953b86c6e3aaf01a1ae923fc0ea0b5212048f5dd",
        )
        .unwrap();
    }

    fn provider() -> JsonRpcClient<HttpTransport> {
        let url = Url::parse(KATANA_ENDPOINT).expect("Invalid Katana endpoint");
        JsonRpcClient::new(HttpTransport::new(url))
    }

    fn paid_fee(tx: &str) -> Result<FieldElement> {
        let client = reqwest::blocking::Client::new();
        let body = format!(
            "{{\"jsonrpc\": \"2.0\",\"method\": \"starknet_getTransactionReceipt\",\"params\": [\"{}\"],\"id\": 1}}",
            tx,
        );

        let mut retries = 0;
        let receipt = loop {
            let res = client
                .post(KATANA_ENDPOINT)
                .body(body.clone())
                .header("Content-Type", "application/json")
                .send()
                .context("Failed to send request")?;

            if res.status() != 200 {
                return Err(anyhow!("Failed to fetch receipt"));
            }

            let receipt: JsonRpcResponse<TransactionReceipt> =
                res.json().context("Failed to parse response")?;

            match receipt {
                JsonRpcResponse::Success { result, .. } => break result,
                JsonRpcResponse::Error { error, .. } => {
                    if retries > 10 {
                        return Err(anyhow!("Transaction {} failed with: {}", tx, error));
                    } else {
                        retries += 1;
                    }
                }
            }

            thread::sleep(Duration::from_millis(50));
        };

        if let TransactionReceipt::Invoke(receipt) = receipt {
            Ok(receipt.actual_fee)
        } else {
            return Err(anyhow!("Not an invoke transaction"));
        }
    }

    #[test]
    fn prepare_test() {
        assert_eq!(
            KEY_PAIRS[0].0, "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973",
            "Katana prefunded accounts are not loaded"
        );
    }

    // does not need proptest, as it doesn't use any input
    #[test]
    fn bench_spawn() {
        let tx = execute("spawn", None).unwrap().0;

        let fee = paid_fee(&tx).unwrap();
        assert!(fee > FieldElement::ONE);
        println!("Tx: {}, fee: {}", tx, fee);
    }

    proptest! {
        #[test]
        fn bench_move(c in "0x[0-3]") {
            let tx = execute("spawn", None).unwrap()
                .then("move", Some(c.clone())).unwrap().tx();

            let fee = paid_fee(&tx).expect("Failed to fetch fee");
            assert!(fee > FieldElement::ONE);
            println!("Data: {} in tx: {}, with fee: {}", c, tx, fee);
        }
    }

    proptest! {
        #[test]
        fn bench_emit(s in "[A-Za-z0-9]{1,31}") {
            let s_hex = s.as_bytes().encode_hex::<String>();

            let tx = execute("bench_emit", Some("0x".to_owned() + &s_hex)).unwrap().tx();

            let fee = paid_fee(&tx).expect("Failed to fetch fee");
            assert!(fee > FieldElement::ONE);
            println!("tx: {}\tfee: {}\tcalldata: {}", tx, fee, s);
        }
    }

    proptest! {
        #[test]
        fn bench_set(s in "[A-Za-z0-9]{1,31}") {
            let s_hex = s.as_bytes().encode_hex::<String>();

            let tx = execute("bench_set", Some("0x".to_owned() + &s_hex)).unwrap().tx();

            let fee = paid_fee(&tx).expect("Failed to fetch fee");
            assert!(fee > FieldElement::ONE);
            println!("tx: {}\tfee: {}\tcalldata: {}", tx, fee, s);
        }
    }

    proptest! {
        #[test]
        fn bench_get(s in "[A-Za-z0-9]{1,31}") {
            let s_hex = s.as_bytes().encode_hex::<String>();

            let tx = execute("bench_set", Some("0x".to_owned() + &s_hex)).unwrap()
                .then("bench_get", None).unwrap().tx();

            let fee = paid_fee(&tx).expect("Failed to fetch fee");
            assert!(fee > FieldElement::ONE);
            println!("tx: {}\tfee: {}\tcalldata: {}", tx, fee, s);
        }
    }

    #[tokio::test]
    async fn test_nonce() {
        let private = FieldElement::from_hex_be(
            "0x319c161623eeb7bb65d443eaf6d3a5954173961922a5d6bf0b100c87503b68f",
        )
        .unwrap();
        let signer = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(private));
        let address = FieldElement::from_hex_be(
            "0x68597f52edc17608661ded82f0dcb69118278541717fba08511b4e58c54e48a",
        )
        .unwrap();

        let provider = provider();
        let chain_id = provider.chain_id().await.unwrap();

        let mut account =
            SingleOwnerAccount::new(provider, signer, address, chain_id, ExecutionEncoding::Legacy);
        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        let calls = vec![Call {
            to: *CONTRACT,
            selector: get_selector_from_name("spawn").unwrap(),
            calldata: vec![],
        }];

        let execution = Execution::new(calls, &account);
        let fee = execution.estimate_fee().await.unwrap();

        let gas = fee.gas_consumed;
        assert!(gas > 0);
    }
}
