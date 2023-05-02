use anyhow::{Ok, Result};
use starknet::core::{crypto::compute_hash_on_elements, types::FieldElement};
use starknet_api::hash::StarkFelt;

const PREFIX_INVOKE: FieldElement = FieldElement::from_mont([
    18443034532770911073,
    18446744073709551615,
    18446744073709551615,
    513398556346534256,
]);

pub fn to_trimmed_hex_string(bytes: &[u8]) -> String {
    let hex_str = hex::encode(bytes);
    let trimmed_hex_str = hex_str.trim_start_matches('0');
    if trimmed_hex_str.is_empty() {
        "0x0".to_string()
    } else {
        format!("0x{}", trimmed_hex_str)
    }
}

pub fn stark_felt_to_field_element(felt: StarkFelt) -> Result<FieldElement> {
    Ok(FieldElement::from_byte_slice_be(felt.bytes())?)
}

pub fn compute_invoke_v1_transaction_hash(
    sender_address: FieldElement,
    calldata: &[FieldElement],
    max_fee: FieldElement,
    chain_id: FieldElement,
    nonce: FieldElement,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_INVOKE,
        FieldElement::ONE, // version
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(calldata),
        max_fee,
        chain_id,
        nonce,
    ])
}
