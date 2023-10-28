#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
mod tests {
    use anyhow::{Context, Result};
    use futures::executor::block_on;
    use hex::ToHex;
    use lazy_static::lazy_static;
    use reqwest::Url;
    use starknet::accounts::{Account, Call, ExecutionEncoding, SingleOwnerAccount};
    use starknet::core::types::{BlockId, BlockTag, FieldElement};
    use starknet::core::utils::get_selector_from_name;
    use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient, Provider};
    use starknet::signers::{LocalWallet, SigningKey};
    use tokio::runtime::Runtime;

    use proptest::prelude::*;
    use tokio::sync::OnceCell;

    const KATANA_ENDPOINT: &str = "http://localhost:5050";
    const CONTRACT_ADDRESS: &str =
        "0x5d69ccf0644b87204e143d2953b86c6e3aaf01a1ae923fc0ea0b5212048f5dd";

    const ACCOUNT_ADDRESS: &str =
        "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973";
    const PRIVATE_KEY: &str = "0x1800000000300000180000000000030000000000003006001800006600";

    type OwnerAccount = SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>;

    lazy_static! {
        static ref CONTRACT: FieldElement = FieldElement::from_hex_be(CONTRACT_ADDRESS).unwrap();
        static ref RUNTIME: Runtime = Runtime::new().unwrap();
    }

    async fn chain_id() -> FieldElement {
        // cache the chain_id
        static CHAIN_ID: OnceCell<FieldElement> = OnceCell::const_new();

        *CHAIN_ID
            .get_or_init(|| async {
                let provider = provider();
                provider.chain_id().await.unwrap()
            })
            .await
    }

    async fn account() -> OwnerAccount {
        let signer = LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            FieldElement::from_hex_be(PRIVATE_KEY).unwrap(),
        ));
        let address = FieldElement::from_hex_be(ACCOUNT_ADDRESS).unwrap();
        let mut account = SingleOwnerAccount::new(
            provider(),
            signer,
            address,
            chain_id().await,
            ExecutionEncoding::Legacy,
        );
        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        account
    }

    fn provider() -> JsonRpcClient<HttpTransport> {
        let url = Url::parse(KATANA_ENDPOINT).expect("Invalid Katana endpoint");
        JsonRpcClient::new(HttpTransport::new(url))
    }

    fn execute(entrypoints_and_calldata: Vec<(&str, Vec<FieldElement>)>) -> Result<u64> {
        let calls = entrypoints_and_calldata
            .into_iter()
            .map(|(name, calldata)| Call {
                to: *CONTRACT,
                selector: get_selector_from_name(name).context("Failed to get selector").unwrap(),
                calldata,
            })
            .collect();

        let provider = provider();

        let _rt = RUNTIME.enter();
        let chain_id =
            block_on(async move { provider.chain_id().await.expect("Couldn't fetch chain_id") });

        let fee = block_on(async move {
            let fee = account()
                .await
                .execute(calls)
                .estimate_fee()
                .await
                .context("Failed to estimate fee")
                .unwrap();

            fee
        });

        Ok(fee.gas_consumed)
    }

    // does not need proptest, as it doesn't use any input
    #[test]
    fn bench_spawn() {
        let fee = execute(vec![("spawn", vec![])]).unwrap();

        assert!(fee > 1);
    }

    proptest! {
        #[test]
        fn bench_move(c in "0x[0-4]") {
            let calls = vec![("spawn", vec![]), ("move", vec![FieldElement::from_hex_be(&c).unwrap()])];
            let fee = execute(calls).unwrap();

            assert!(fee > 1);
            println!("Data: {} , with fee: {}", c, fee);
        }
    }

    proptest! {
        #[test]
        fn bench_emit(s in "[A-Za-z0-9]{1,31}") {
            let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

            let fee = execute(vec![("bench_emit", vec![s_hex])]).unwrap();

            assert!(fee > 1);
            println!("fee: {}\tcalldata: {}", fee, s);
        }
    }

    proptest! {
        #[test]
        fn bench_set(s in "[A-Za-z0-9]{1,31}") {
            let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();

            let fee = execute(vec![("bench_set", vec![s_hex])]).unwrap();

            assert!(fee > 1);
            println!("Fee: {}\tcalldata: {}", fee, s);
        }
    }

    proptest! {
        #[test]
        fn bench_get(s in "[A-Za-z0-9]{1,31}") {
            let s_hex = FieldElement::from_hex_be(&format!("0x{}", s.as_bytes().encode_hex::<String>())).unwrap();
            let calls = vec![("bench_set", vec![s_hex]), ("bench_get", vec![])];

            let fee = execute(calls).unwrap();

            assert!(fee > 1);
            println!("Fee: {}\tcalldata: {}", fee, s);
        }
    }
}
