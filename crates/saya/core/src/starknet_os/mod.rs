//! Starknet OS types.
// SNOS is based on blockifier, which is not in sync with
// current primitives.
// And SNOS is for now not used. This work must be resume once
// SNOS is actualized.
// mod felt;
// pub mod input;
// pub mod transaction;

use itertools::chain;
use starknet::accounts::{Account, Call, ExecutionEncoding, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, FieldElement};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};
use url::Url;

// will need to be read from the environment for chains other than sepoia
pub const STARKNET_URL: &str = "https://free-rpc.nethermind.io/sepolia-juno/v0_6";
pub const CHAIN_ID: &str = "0x00000000000000000000000000000000000000000000534e5f5345504f4c4941";
pub const SIGNER_ADDRESS: &str =
    "0x76372bcb1d993b9ab059e542a93004962fb70d743b0f10e611df9ffe13c6d64";
pub const SIGNER_KEY: &str = "0x710d3218ae70bf7ec580c620ec81e601a6258ceec2494c4261f916f42667000";

lazy_static::lazy_static!(
    pub static ref STARKNET_ACCOUNT: SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet> = {
        let provider = JsonRpcClient::new(HttpTransport::new(
            Url::parse(STARKNET_URL).unwrap(),
        ));

        let signer = FieldElement::from_hex_be(SIGNER_KEY).expect("invalid signer hex");
        let signer = LocalWallet::from(SigningKey::from_secret_scalar(signer));

        let address = FieldElement::from_hex_be(SIGNER_ADDRESS).expect("invalid signer address");
        let chain_id = FieldElement::from_hex_be(CHAIN_ID).expect("invalid chain id");

        let mut account = SingleOwnerAccount::new(provider, signer, address, chain_id, ExecutionEncoding::Legacy);
        account.set_block_id(BlockId::Tag(BlockTag::Pending));
        account
    };
);

pub async fn starknet_apply_diffs(
    new_state: Vec<FieldElement>,
    program_output: Vec<FieldElement>,
) -> anyhow::Result<String> {
    let calldata = chain![
        vec![FieldElement::from_dec_str(&new_state.len().to_string()).unwrap()].into_iter(),
        new_state.into_iter(),
        vec![FieldElement::from_dec_str(&program_output.len().to_string()).unwrap()].into_iter(),
        program_output.into_iter()
    ]
    .collect();
    let tx = STARKNET_ACCOUNT
        .execute(vec![Call {
            to: FieldElement::from_hex_be(
                "0x4c4e3d09d5db141773381a11dff7259b99283cc9c6558705cb955f919b2af36",
            )
            .expect("invalid verifier address"),
            selector: get_selector_from_name("verify_and_register_fact").expect("invalid selector"),
            calldata: calldata,
        }])
        .max_fee(starknet::macros::felt!("1000000000000000")) // sometimes failing without this line 
        .send()
        .await?;

    Ok(format!("{:#x}", tx.transaction_hash))
}
