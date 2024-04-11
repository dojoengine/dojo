use crate::starknet_os::STARKNET_ACCOUNT;
use starknet::accounts::{Account, Call};
use starknet::core::types::FieldElement;
use starknet::core::utils::get_selector_from_name;

pub async fn starknet_verify(serialized_proof: Vec<FieldElement>) -> anyhow::Result<String> {
    let tx = STARKNET_ACCOUNT
        .execute(vec![Call {
            to: FieldElement::from_hex_be(
                "0x11471d2f05904ba5ec06ea9882df412b452e732588ad7773793a0ef470f2599",
            )
            .expect("invalid verifier address"),
            selector: get_selector_from_name("verify_and_register_fact").expect("invalid selector"),
            calldata: serialized_proof,
        }])
        .max_fee(starknet::macros::felt!("1000000000000000")) // sometimes failing without this line 
        .send()
        .await?;

    Ok(format!("{:#x}", tx.transaction_hash))
}
