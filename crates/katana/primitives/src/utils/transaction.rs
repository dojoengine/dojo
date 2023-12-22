use ethers::types::H256;
use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::types::MsgToL1;

use crate::FieldElement;

/// 2^ 128
const QUERY_VERSION_OFFSET: FieldElement = FieldElement::from_mont([
    18446744073700081665,
    17407,
    18446744073709551584,
    576460752142434320,
]);

/// Cairo string for "invoke"
const PREFIX_INVOKE: FieldElement = FieldElement::from_mont([
    18443034532770911073,
    18446744073709551615,
    18446744073709551615,
    513398556346534256,
]);

/// Cairo string for "declare"
const PREFIX_DECLARE: FieldElement = FieldElement::from_mont([
    17542456862011667323,
    18446744073709551615,
    18446744073709551615,
    191557713328401194,
]);

/// Cairo string for "deploy_account"
const PREFIX_DEPLOY_ACCOUNT: FieldElement = FieldElement::from_mont([
    3350261884043292318,
    18443211694809419988,
    18446744073709551615,
    461298303000467581,
]);

/// Cairo string for "l1_handler"
const PREFIX_L1_HANDLER: FieldElement = FieldElement::from_mont([
    1365666230910873368,
    18446744073708665300,
    18446744073709551615,
    157895833347907735,
]);

/// Compute the hash of a V1 DeployAccount transaction.
#[allow(clippy::too_many_arguments)]
pub fn compute_deploy_account_v1_tx_hash(
    contract_address: FieldElement,
    constructor_calldata: &[FieldElement],
    class_hash: FieldElement,
    salt: FieldElement,
    max_fee: u128,
    chain_id: FieldElement,
    nonce: FieldElement,
    is_query: bool,
) -> FieldElement {
    let calldata_to_hash = [&[class_hash, salt], constructor_calldata].concat();

    compute_hash_on_elements(&[
        PREFIX_DEPLOY_ACCOUNT,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::ONE } else { FieldElement::ONE }, /* version */
        contract_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&calldata_to_hash),
        max_fee.into(),
        chain_id,
        nonce,
    ])
}

/// Compute the hash of a V1 Declare transaction.
pub fn compute_declare_v1_tx_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    max_fee: u128,
    chain_id: FieldElement,
    nonce: FieldElement,
    is_query: bool,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::ONE } else { FieldElement::ONE }, /* version */
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&[class_hash]),
        max_fee.into(),
        chain_id,
        nonce,
    ])
}

/// Compute the hash of a V2 Declare transaction.
pub fn compute_declare_v2_tx_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    max_fee: u128,
    chain_id: FieldElement,
    nonce: FieldElement,
    compiled_class_hash: FieldElement,
    is_query: bool,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::TWO } else { FieldElement::TWO }, /* version */
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&[class_hash]),
        max_fee.into(),
        chain_id,
        nonce,
        compiled_class_hash,
    ])
}

/// Compute the hash of a V1 Invoke transaction.
pub fn compute_invoke_v1_tx_hash(
    sender_address: FieldElement,
    calldata: &[FieldElement],
    max_fee: u128,
    chain_id: FieldElement,
    nonce: FieldElement,
    is_query: bool,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_INVOKE,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::ONE } else { FieldElement::ONE }, /* version */
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(calldata),
        max_fee.into(),
        chain_id,
        nonce,
    ])
}

/// Computes the hash of a L1 handler transaction
/// from the fields involved in the computation,
/// as felts values.
pub fn compute_l1_handler_tx_hash(
    version: FieldElement,
    contract_address: FieldElement,
    entry_point_selector: FieldElement,
    calldata: &[FieldElement],
    chain_id: FieldElement,
    nonce: FieldElement,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_L1_HANDLER,
        version,
        contract_address,
        entry_point_selector,
        compute_hash_on_elements(calldata),
        FieldElement::ZERO, // No fee on L2 for L1 handler tx
        chain_id,
        nonce,
    ])
}

/// Computes the hash of a L1 message.
///
/// The hash that is used to consume the message in L1.
pub fn compute_l1_message_hash(
    from_address: FieldElement,
    to_address: FieldElement,
    payload: &[FieldElement],
) -> H256 {
    let msg = MsgToL1 { from_address, to_address, payload: payload.to_vec() };

    H256::from_slice(msg.hash().as_bytes())
}

#[cfg(test)]
mod tests {
    use starknet::core::chain_id;

    use super::*;

    #[test]
    fn test_compute_deploy_account_v1_transaction_hash() {
        let contract_address = FieldElement::from_hex_be(
            "0x0617e350ebed9897037bdef9a09af65049b85ed2e4c9604b640f34bffa152149",
        )
        .unwrap();
        let constructor_calldata = vec![
            FieldElement::from_hex_be(
                "0x33434ad846cdd5f23eb73ff09fe6fddd568284a0fb7d1be20ee482f044dabe2",
            )
            .unwrap(),
            FieldElement::from_hex_be(
                "0x79dc0da7c54b95f10aa182ad0a46400db63156920adb65eca2654c0945a463",
            )
            .unwrap(),
            FieldElement::from_hex_be("0x2").unwrap(),
            FieldElement::from_hex_be(
                "0x43a8fbe19d5ace41a2328bb870143241831180eb3c3c48096642d63709c3096",
            )
            .unwrap(),
            FieldElement::from_hex_be("0x0").unwrap(),
        ];
        let class_hash = FieldElement::from_hex_be(
            "0x025ec026985a3bf9d0cc1fe17326b245dfdc3ff89b8fde106542a3ea56c5a918",
        )
        .unwrap();
        let salt = FieldElement::from_hex_be(
            "0x43a8fbe19d5ace41a2328bb870143241831180eb3c3c48096642d63709c3096",
        )
        .unwrap();
        let max_fee = FieldElement::from_hex_be("0x38d7ea4c68000").unwrap();
        let chain_id = chain_id::MAINNET;
        let nonce = FieldElement::ZERO;

        let hash = compute_deploy_account_v1_tx_hash(
            contract_address,
            &constructor_calldata,
            class_hash,
            salt,
            max_fee.try_into().unwrap(),
            chain_id,
            nonce,
            false,
        );

        assert_eq!(
            hash,
            FieldElement::from_hex_be(
                "0x3d013d17c20a5db05d5c2e06c948a4e0bf5ea5b851b15137316533ec4788b6b"
            )
            .unwrap()
        );
    }
}
