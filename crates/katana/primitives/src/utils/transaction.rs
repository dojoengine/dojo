use alloy_primitives::B256;
use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::types::{DataAvailabilityMode, EthAddress, MsgToL1, MsgToL2, ResourceBounds};
use starknet_crypto::poseidon_hash_many;

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
    sender_address: FieldElement,
    constructor_calldata: &[FieldElement],
    class_hash: FieldElement,
    contract_address_salt: FieldElement,
    max_fee: u128,
    chain_id: FieldElement,
    nonce: FieldElement,
    is_query: bool,
) -> FieldElement {
    let calldata_to_hash = [&[class_hash, contract_address_salt], constructor_calldata].concat();

    compute_hash_on_elements(&[
        PREFIX_DEPLOY_ACCOUNT,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::ONE } else { FieldElement::ONE }, /* version */
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&calldata_to_hash),
        max_fee.into(),
        chain_id,
        nonce,
    ])
}

/// Compute the hash of a V1 DeployAccount transaction.
#[allow(clippy::too_many_arguments)]
pub fn compute_deploy_account_v3_tx_hash(
    contract_address: FieldElement,
    constructor_calldata: &[FieldElement],
    class_hash: FieldElement,
    contract_address_salt: FieldElement,
    tip: u64,
    l1_gas_bounds: &ResourceBounds,
    l2_gas_bounds: &ResourceBounds,
    paymaster_data: &[FieldElement],
    chain_id: FieldElement,
    nonce: FieldElement,
    nonce_da_mode: &DataAvailabilityMode,
    fee_da_mode: &DataAvailabilityMode,
    is_query: bool,
) -> FieldElement {
    let data_hash = poseidon_hash_many(constructor_calldata);

    poseidon_hash_many(&[
        PREFIX_INVOKE,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::THREE } else { FieldElement::THREE }, /* version */
        contract_address,
        hash_fee_fields(tip, l1_gas_bounds, l2_gas_bounds),
        poseidon_hash_many(paymaster_data),
        chain_id,
        nonce,
        data_hash,
        encode_da_mode(nonce_da_mode, fee_da_mode),
        class_hash,
        contract_address_salt,
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

/// Compute the hash of a V3 Declare transaction.
#[allow(clippy::too_many_arguments)]
pub fn compute_declare_v3_tx_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    compiled_class_hash: FieldElement,
    tip: u64,
    l1_gas_bounds: &ResourceBounds,
    l2_gas_bounds: &ResourceBounds,
    paymaster_data: &[FieldElement],
    chain_id: FieldElement,
    nonce: FieldElement,
    nonce_da_mode: &DataAvailabilityMode,
    fee_da_mode: &DataAvailabilityMode,
    account_deployment_data: &[FieldElement],
    is_query: bool,
) -> FieldElement {
    let data_hash = poseidon_hash_many(account_deployment_data);

    poseidon_hash_many(&[
        PREFIX_INVOKE,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::THREE } else { FieldElement::THREE }, /* version */
        sender_address,
        hash_fee_fields(tip, l1_gas_bounds, l2_gas_bounds),
        poseidon_hash_many(paymaster_data),
        chain_id,
        nonce,
        data_hash,
        encode_da_mode(nonce_da_mode, fee_da_mode),
        class_hash,
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

/// Compute the hash of a V1 Invoke transaction.
#[allow(clippy::too_many_arguments)]
pub fn compute_invoke_v3_tx_hash(
    sender_address: FieldElement,
    calldata: &[FieldElement],
    tip: u64,
    l1_gas_bounds: &ResourceBounds,
    l2_gas_bounds: &ResourceBounds,
    paymaster_data: &[FieldElement],
    chain_id: FieldElement,
    nonce: FieldElement,
    nonce_da_mode: &DataAvailabilityMode,
    fee_da_mode: &DataAvailabilityMode,
    account_deployment_data: &[FieldElement],
    is_query: bool,
) -> FieldElement {
    let data_hash = poseidon_hash_many(&[
        poseidon_hash_many(account_deployment_data),
        poseidon_hash_many(calldata),
    ]);

    poseidon_hash_many(&[
        PREFIX_INVOKE,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::THREE } else { FieldElement::THREE }, /* version */
        sender_address,
        hash_fee_fields(tip, l1_gas_bounds, l2_gas_bounds),
        poseidon_hash_many(paymaster_data),
        chain_id,
        nonce,
        data_hash,
        encode_da_mode(nonce_da_mode, fee_da_mode),
    ])
}

/// Computes the hash of a L1 handler transaction
/// from the fields involved in the computation,
/// as felts values.
///
/// The [Starknet docs] seem to be different than how it's implemented by Starknet node client
/// implementations - [Juno], [Pathfinder], and [Deoxys]. So, we follow those implementations
/// instead.
///
/// [Juno]: https://github.com/NethermindEth/juno/blob/d9e64106a3a6d81d217d3c8baf28749f4f0bdd71/core/transaction.go#L561-L569
/// [Pathfinder]: https://github.com/eqlabs/pathfinder/blob/677fd40fbae7b5b659bf169e56f055c59cbb3f52/crates/common/src/transaction.rs#L556
/// [Deoxys]: https://github.com/KasarLabs/deoxys/blob/82c49acdaa1167bc8dc67a3f6ad3d6856c6c7e89/crates/primitives/transactions/src/compute_hash.rs#L142-L151
/// [Starknet docs]: https://docs.starknet.io/architecture-and-concepts/network-architecture/messaging-mechanism/#hashing_l1-l2
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

/// Computes the hash of a L2 to L1 message.
///
/// The hash that is used to consume the message in L1.
pub fn compute_l2_to_l1_message_hash(
    from_address: FieldElement,
    to_address: FieldElement,
    payload: &[FieldElement],
) -> B256 {
    let msg = MsgToL1 { from_address, to_address, payload: payload.to_vec() };
    B256::from_slice(msg.hash().as_bytes())
}

// TODO: standardize the usage of eth types. prefer to use alloy (for its convenience) instead of
// starknet-rs's types.
/// Computes the hash of a L1 to L2 message.
pub fn compute_l1_to_l2_message_hash(
    from_address: EthAddress,
    to_address: FieldElement,
    selector: FieldElement,
    payload: &[FieldElement],
    nonce: u64,
) -> B256 {
    let msg = MsgToL2 { from_address, to_address, selector, payload: payload.to_vec(), nonce };
    B256::from_slice(msg.hash().as_bytes())
}

fn encode_gas_bound(name: &[u8], bound: &ResourceBounds) -> FieldElement {
    let mut buffer = [0u8; 32];
    let (remainder, max_price) = buffer.split_at_mut(128 / 8);
    let (gas_kind, max_amount) = remainder.split_at_mut(64 / 8);

    let padding = gas_kind.len() - name.len();
    gas_kind[padding..].copy_from_slice(name);
    max_amount.copy_from_slice(&bound.max_amount.to_be_bytes());
    max_price.copy_from_slice(&bound.max_price_per_unit.to_be_bytes());

    FieldElement::from_bytes_be(&buffer).expect("Packed resource should fit into felt")
}

fn hash_fee_fields(
    tip: u64,
    l1_gas_bounds: &ResourceBounds,
    l2_gas_bounds: &ResourceBounds,
) -> FieldElement {
    poseidon_hash_many(&[
        tip.into(),
        encode_gas_bound(b"L1_GAS", l1_gas_bounds),
        encode_gas_bound(b"L2_GAS", l2_gas_bounds),
    ])
}

fn encode_da_mode(
    nonce_da_mode: &DataAvailabilityMode,
    fee_da_mode: &DataAvailabilityMode,
) -> FieldElement {
    let nonce = (*nonce_da_mode as u64) << 32;
    let fee = *fee_da_mode as u64;
    FieldElement::from(nonce + fee)
}

#[cfg(test)]
mod tests {
    use starknet::core::chain_id;
    use starknet::macros::felt;

    use super::*;

    #[test]
    fn test_compute_deploy_account_v1_tx_hash() {
        // Starknet mainnet tx hash: https://voyager.online/tx/0x3d013d17c20a5db05d5c2e06c948a4e0bf5ea5b851b15137316533ec4788b6b
        let expected_hash =
            felt!("0x3d013d17c20a5db05d5c2e06c948a4e0bf5ea5b851b15137316533ec4788b6b");

        let contract_address =
            felt!("0x0617e350ebed9897037bdef9a09af65049b85ed2e4c9604b640f34bffa152149");
        let constructor_calldata = vec![
            felt!("0x33434ad846cdd5f23eb73ff09fe6fddd568284a0fb7d1be20ee482f044dabe2"),
            felt!("0x79dc0da7c54b95f10aa182ad0a46400db63156920adb65eca2654c0945a463"),
            felt!("0x2"),
            felt!("0x43a8fbe19d5ace41a2328bb870143241831180eb3c3c48096642d63709c3096"),
            felt!("0x0"),
        ];
        let class_hash = felt!("0x25ec026985a3bf9d0cc1fe17326b245dfdc3ff89b8fde106542a3ea56c5a918");
        let salt = felt!("0x43a8fbe19d5ace41a2328bb870143241831180eb3c3c48096642d63709c3096");
        let max_fee = felt!("0x38d7ea4c68000");
        let chain_id = chain_id::MAINNET;
        let nonce = FieldElement::ZERO;

        let actual_hash = compute_deploy_account_v1_tx_hash(
            contract_address,
            &constructor_calldata,
            class_hash,
            salt,
            max_fee.try_into().unwrap(),
            chain_id,
            nonce,
            false,
        );

        assert_eq!(actual_hash, expected_hash);
    }
}
