use starknet::core::crypto::compute_hash_on_elements;

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
pub fn compute_deploy_account_v1_transaction_hash(
    contract_address: FieldElement,
    constructor_calldata: &[FieldElement],
    class_hash: FieldElement,
    salt: FieldElement,
    max_fee: FieldElement,
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
        max_fee,
        chain_id,
        nonce,
    ])
}

/// Compute the hash of a V1 Declare transaction.
pub fn compute_declare_v1_transaction_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    max_fee: FieldElement,
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
        max_fee,
        chain_id,
        nonce,
    ])
}

/// Compute the hash of a V2 Declare transaction.
pub fn compute_declare_v2_transaction_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    max_fee: FieldElement,
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
        max_fee,
        chain_id,
        nonce,
        compiled_class_hash,
    ])
}

/// Compute the hash of a V1 Invoke transaction.
pub fn compute_invoke_v1_transaction_hash(
    sender_address: FieldElement,
    calldata: &[FieldElement],
    max_fee: FieldElement,
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
        max_fee,
        chain_id,
        nonce,
    ])
}

/// Computes the hash of a L1 handler transaction
/// from the fields involved in the computation,
/// as felts values.
pub fn compute_l1_handler_transaction_hash(
    version: FieldElement,
    contract_address: FieldElement,
    entry_point_selector: FieldElement,
    calldata: &[FieldElement],
    chain_id: FieldElement,
    nonce: FieldElement,
) -> FieldElement {
    // No fee on L2 for L1 handler transaction.
    let fee = FieldElement::ZERO;

    compute_hash_on_elements(&[
        PREFIX_L1_HANDLER,
        version,
        contract_address,
        entry_point_selector,
        compute_hash_on_elements(calldata),
        fee,
        chain_id,
        nonce,
    ])
}
