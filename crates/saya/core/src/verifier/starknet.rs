use starknet::{
    accounts::{Account, Call, ExecutionEncoding, SingleOwnerAccount},
    core::{types::FieldElement, utils::get_selector_from_name},
    providers::{jsonrpc::HttpTransport, JsonRpcClient},
    signers::{LocalWallet, SigningKey},
};
use url::Url;

// will need to be read from the environment for chains other than sepoia
const STARKNET_URL: &str = "https://free-rpc.nethermind.io/sepolia-juno/v0_6";
const CHAIN_ID: &str = "0x00000000000000000000000000000000000000000000534e5f5345504f4c4941";
const SIGNER_ADDRESS: &str = "0x76372bcb1d993b9ab059e542a93004962fb70d743b0f10e611df9ffe13c6d64";
const SIGNER_KEY: &str = "0x710d3218ae70bf7ec580c620ec81e601a6258ceec2494c4261f916f42667000";

lazy_static::lazy_static!(
    static ref STARKNET_ACCOUNT: SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> = {
        let provider = JsonRpcClient::new(HttpTransport::new(
            Url::parse(STARKNET_URL).unwrap(),
        ));

        let signer = FieldElement::from_hex_be(SIGNER_KEY).expect("invalid signer hex");
        let signer = LocalWallet::from(SigningKey::from_secret_scalar(signer));

        let address = FieldElement::from_hex_be(SIGNER_ADDRESS).expect("invalid signer address");
        let chain_id = FieldElement::from_hex_be(CHAIN_ID).expect("invalid chain id");

        SingleOwnerAccount::new(provider, signer, address, chain_id, ExecutionEncoding::Legacy)
    };

);

pub async fn starknet_verify(serialized_proof: Vec<FieldElement>) -> anyhow::Result<String> {
    let tx = STARKNET_ACCOUNT
        .execute(vec![Call {
            to: FieldElement::from_hex_be(
                "0x1b9c4e973ca9af0456eb6ae4c4576c5134905d8a560e0dfa1b977359e2c40ec",
            )
            .expect("invalid verifier address"),
            selector: get_selector_from_name("verify_and_register_fact").expect("invalid selector"),
            calldata: serialized_proof,
        }])
        .send()
        .await?;

    Ok(tx.transaction_hash.to_string())
}
