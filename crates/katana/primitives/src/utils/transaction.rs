use alloy_primitives::B256;
use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::types::{EthAddress, MsgToL1, MsgToL2};
use starknet_crypto::poseidon_hash_many;

use crate::da::DataAvailabilityMode;
use crate::fee::ResourceBounds;
use crate::Felt;

/// 2^ 128
const QUERY_VERSION_OFFSET: Felt =
    Felt::from_raw([576460752142434320, 18446744073709551584, 17407, 18446744073700081665]);

/// Cairo string for "invoke"
const PREFIX_INVOKE: Felt = Felt::from_raw([
    513398556346534256,
    18446744073709551615,
    18446744073709551615,
    18443034532770911073,
]);

/// Cairo string for "declare"
const PREFIX_DECLARE: Felt = Felt::from_raw([
    191557713328401194,
    18446744073709551615,
    18446744073709551615,
    17542456862011667323,
]);

/// Cairo string for "deploy_account"
const PREFIX_DEPLOY_ACCOUNT: Felt = Felt::from_raw([
    461298303000467581,
    18446744073709551615,
    18443211694809419988,
    3350261884043292318,
]);

/// Cairo string for "l1_handler"
const PREFIX_L1_HANDLER: Felt = Felt::from_raw([
    157895833347907735,
    18446744073709551615,
    18446744073708665300,
    1365666230910873368,
]);

/// Compute the hash of a V1 DeployAccount transaction.
#[allow(clippy::too_many_arguments)]
pub fn compute_deploy_account_v1_tx_hash(
    sender_address: Felt,
    constructor_calldata: &[Felt],
    class_hash: Felt,
    contract_address_salt: Felt,
    max_fee: u128,
    chain_id: Felt,
    nonce: Felt,
    is_query: bool,
) -> Felt {
    let calldata_to_hash = [&[class_hash, contract_address_salt], constructor_calldata].concat();

    compute_hash_on_elements(&[
        PREFIX_DEPLOY_ACCOUNT,
        if is_query { QUERY_VERSION_OFFSET + Felt::ONE } else { Felt::ONE }, // version
        sender_address,
        Felt::ZERO, // entry_point_selector
        compute_hash_on_elements(&calldata_to_hash),
        max_fee.into(),
        chain_id,
        nonce,
    ])
}

/// Compute the hash of a V3 DeployAccount transaction.
#[allow(clippy::too_many_arguments)]
pub fn compute_deploy_account_v3_tx_hash(
    contract_address: Felt,
    constructor_calldata: &[Felt],
    class_hash: Felt,
    contract_address_salt: Felt,
    tip: u64,
    l1_gas_bounds: &ResourceBounds,
    l2_gas_bounds: &ResourceBounds,
    paymaster_data: &[Felt],
    chain_id: Felt,
    nonce: Felt,
    nonce_da_mode: &DataAvailabilityMode,
    fee_da_mode: &DataAvailabilityMode,
    is_query: bool,
) -> Felt {
    poseidon_hash_many(&[
        PREFIX_DEPLOY_ACCOUNT,
        if is_query { QUERY_VERSION_OFFSET + Felt::THREE } else { Felt::THREE }, // version
        contract_address,
        hash_fee_fields(tip, l1_gas_bounds, l2_gas_bounds),
        poseidon_hash_many(paymaster_data),
        chain_id,
        nonce,
        encode_da_mode(nonce_da_mode, fee_da_mode),
        poseidon_hash_many(constructor_calldata),
        class_hash,
        contract_address_salt,
    ])
}

/// Compute the hash of a V0 Declare transaction.
///
/// Reference: https://github.com/dojoengine/sequencer/blob/6f72e5cc30cae2a0db72b709ee6375ba863cfc58/crates/starknet_api/src/transaction_hash.rs#L471-L488
pub fn compute_declare_v0_tx_hash(
    sender_address: Felt,
    class_hash: Felt,
    max_fee: u128,
    chain_id: Felt,
    is_query: bool,
) -> Felt {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        if is_query { QUERY_VERSION_OFFSET + Felt::ZERO } else { Felt::ZERO }, // version
        sender_address,
        Felt::ZERO, // entry_point_selector
        compute_hash_on_elements(&[]),
        max_fee.into(),
        chain_id,
        class_hash,
    ])
}

/// Compute the hash of a V1 Declare transaction.
pub fn compute_declare_v1_tx_hash(
    sender_address: Felt,
    class_hash: Felt,
    max_fee: u128,
    chain_id: Felt,
    nonce: Felt,
    is_query: bool,
) -> Felt {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        if is_query { QUERY_VERSION_OFFSET + Felt::ONE } else { Felt::ONE }, // version
        sender_address,
        Felt::ZERO, // entry_point_selector
        compute_hash_on_elements(&[class_hash]),
        max_fee.into(),
        chain_id,
        nonce,
    ])
}

/// Compute the hash of a V2 Declare transaction.
pub fn compute_declare_v2_tx_hash(
    sender_address: Felt,
    class_hash: Felt,
    max_fee: u128,
    chain_id: Felt,
    nonce: Felt,
    compiled_class_hash: Felt,
    is_query: bool,
) -> Felt {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        if is_query { QUERY_VERSION_OFFSET + Felt::TWO } else { Felt::TWO }, // version
        sender_address,
        Felt::ZERO, // entry_point_selector
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
    sender_address: Felt,
    class_hash: Felt,
    compiled_class_hash: Felt,
    tip: u64,
    l1_gas_bounds: &ResourceBounds,
    l2_gas_bounds: &ResourceBounds,
    paymaster_data: &[Felt],
    chain_id: Felt,
    nonce: Felt,
    nonce_da_mode: &DataAvailabilityMode,
    fee_da_mode: &DataAvailabilityMode,
    account_deployment_data: &[Felt],
    is_query: bool,
) -> Felt {
    poseidon_hash_many(&[
        PREFIX_DECLARE,
        if is_query { QUERY_VERSION_OFFSET + Felt::THREE } else { Felt::THREE }, // version
        sender_address,
        hash_fee_fields(tip, l1_gas_bounds, l2_gas_bounds),
        poseidon_hash_many(paymaster_data),
        chain_id,
        nonce,
        encode_da_mode(nonce_da_mode, fee_da_mode),
        poseidon_hash_many(account_deployment_data),
        class_hash,
        compiled_class_hash,
    ])
}

/// Compute the hash of a V1 Invoke transaction.
pub fn compute_invoke_v1_tx_hash(
    sender_address: Felt,
    calldata: &[Felt],
    max_fee: u128,
    chain_id: Felt,
    nonce: Felt,
    is_query: bool,
) -> Felt {
    compute_hash_on_elements(&[
        PREFIX_INVOKE,
        if is_query { QUERY_VERSION_OFFSET + Felt::ONE } else { Felt::ONE }, // version
        sender_address,
        Felt::ZERO, // entry_point_selector
        compute_hash_on_elements(calldata),
        max_fee.into(),
        chain_id,
        nonce,
    ])
}

/// Compute the hash of a V3 Invoke transaction.
#[allow(clippy::too_many_arguments)]
pub fn compute_invoke_v3_tx_hash(
    sender_address: Felt,
    calldata: &[Felt],
    tip: u64,
    l1_gas_bounds: &ResourceBounds,
    l2_gas_bounds: &ResourceBounds,
    paymaster_data: &[Felt],
    chain_id: Felt,
    nonce: Felt,
    nonce_da_mode: &DataAvailabilityMode,
    fee_da_mode: &DataAvailabilityMode,
    account_deployment_data: &[Felt],
    is_query: bool,
) -> Felt {
    poseidon_hash_many(&[
        PREFIX_INVOKE,
        if is_query { QUERY_VERSION_OFFSET + Felt::THREE } else { Felt::THREE }, // version
        sender_address,
        hash_fee_fields(tip, l1_gas_bounds, l2_gas_bounds),
        poseidon_hash_many(paymaster_data),
        chain_id,
        nonce,
        encode_da_mode(nonce_da_mode, fee_da_mode),
        poseidon_hash_many(account_deployment_data),
        poseidon_hash_many(calldata),
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
    version: Felt,
    contract_address: Felt,
    entry_point_selector: Felt,
    calldata: &[Felt],
    chain_id: Felt,
    nonce: Felt,
) -> Felt {
    compute_hash_on_elements(&[
        PREFIX_L1_HANDLER,
        version,
        contract_address,
        entry_point_selector,
        compute_hash_on_elements(calldata),
        Felt::ZERO, // No fee on L2 for L1 handler tx
        chain_id,
        nonce,
    ])
}

/// Computes the hash of a L2 to L1 message.
///
/// The hash that is used to consume the message in L1.
pub fn compute_l2_to_l1_message_hash(
    from_address: Felt,
    to_address: Felt,
    payload: &[Felt],
) -> B256 {
    let msg = MsgToL1 { from_address, to_address, payload: payload.to_vec() };
    B256::from_slice(msg.hash().as_bytes())
}

// TODO: standardize the usage of eth types. prefer to use alloy (for its convenience) instead of
// starknet-rs's types.
/// Computes the hash of a L1 to L2 message.
pub fn compute_l1_to_l2_message_hash(
    from_address: EthAddress,
    to_address: Felt,
    selector: Felt,
    payload: &[Felt],
    nonce: u64,
) -> B256 {
    let msg = MsgToL2 { from_address, to_address, selector, payload: payload.to_vec(), nonce };
    B256::from_slice(msg.hash().as_bytes())
}

fn encode_gas_bound(name: &[u8], bound: &ResourceBounds) -> Felt {
    let mut buffer = [0u8; 32];
    let (remainder, max_price) = buffer.split_at_mut(128 / 8);
    let (gas_kind, max_amount) = remainder.split_at_mut(64 / 8);

    let padding = gas_kind.len() - name.len();
    gas_kind[padding..].copy_from_slice(name);
    max_amount.copy_from_slice(&bound.max_amount.to_be_bytes());
    max_price.copy_from_slice(&bound.max_price_per_unit.to_be_bytes());

    Felt::from_bytes_be(&buffer)
}

fn hash_fee_fields(
    tip: u64,
    l1_gas_bounds: &ResourceBounds,
    l2_gas_bounds: &ResourceBounds,
) -> Felt {
    poseidon_hash_many(&[
        tip.into(),
        encode_gas_bound(b"L1_GAS", l1_gas_bounds),
        encode_gas_bound(b"L2_GAS", l2_gas_bounds),
    ])
}

fn encode_da_mode(
    nonce_da_mode: &DataAvailabilityMode,
    fee_da_mode: &DataAvailabilityMode,
) -> Felt {
    let nonce = (*nonce_da_mode as u64) << 32;
    let fee = *fee_da_mode as u64;
    Felt::from(nonce + fee)
}

#[cfg(test)]
mod tests {
    use num_traits::ToPrimitive;
    use starknet::macros::{felt, short_string};

    use super::*;
    use crate::chain::ChainId;

    #[test]
    fn test_query_version_offset() {
        // 2^ 128
        assert_eq!(QUERY_VERSION_OFFSET, Felt::TWO.pow(128u8));
    }

    #[test]
    fn test_prefix_constants() {
        assert_eq!(PREFIX_INVOKE, short_string!("invoke"));
        assert_eq!(PREFIX_DECLARE, short_string!("declare"));
        assert_eq!(PREFIX_DEPLOY_ACCOUNT, short_string!("deploy_account"));
        assert_eq!(PREFIX_L1_HANDLER, short_string!("l1_handler"));
    }

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
        let chain_id = ChainId::MAINNET.id();
        let nonce = Felt::ZERO;

        let actual_hash = compute_deploy_account_v1_tx_hash(
            contract_address,
            &constructor_calldata,
            class_hash,
            salt,
            max_fee.to_u128().unwrap(),
            chain_id,
            nonce,
            false,
        );

        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_deploy_account_v3_tx_hash() {
        // Starknet mainnet tx hash: https://voyager.online/tx/0x1b4e364a51dde3b7c696d908c7139244691eccb4c5bce54c874cb5654c053f0
        let expected_hash =
            felt!("0x1b4e364a51dde3b7c696d908c7139244691eccb4c5bce54c874cb5654c053f0");

        let contract_address =
            felt!("0x062e2b954f8ade24b5c901330a984b165a1b7681e8bfd5f6de5bbac937f4ccee");
        let constructor_calldata = vec![
            felt!("0x0"),
            felt!("0x74a02936feda8279d6df2c6ca0991281674fa028fed1990ad9ad460509fa411"),
            felt!("0x1"),
        ];
        let class_hash =
            felt!("0x036078334509b514626504edc9fb252328d1a240e4e948bef8d0c08dff45927f");
        let contract_address_salt =
            felt!("0x74a02936feda8279d6df2c6ca0991281674fa028fed1990ad9ad460509fa411");
        let tip = 0;
        let l1_gas_bounds = ResourceBounds { max_amount: 0x29, max_price_per_unit: 0x16b812d3fa41 };
        let l2_gas_bounds = ResourceBounds { max_amount: 0x0, max_price_per_unit: 0x0 };
        let paymaster_data = vec![];
        let chain_id = ChainId::MAINNET.id();
        let nonce = felt!("0x0");
        let nonce_da_mode = &DataAvailabilityMode::L1;
        let fee_da_mode = &DataAvailabilityMode::L1;

        let actual_hash = compute_deploy_account_v3_tx_hash(
            contract_address,
            &constructor_calldata,
            class_hash,
            contract_address_salt,
            tip,
            &l1_gas_bounds,
            &l2_gas_bounds,
            &paymaster_data,
            chain_id,
            nonce,
            nonce_da_mode,
            fee_da_mode,
            false,
        );

        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_declare_v1_tx_hash() {
        // Starknet mainnet tx hash: https://voyager.online/tx/0x1f01dd957c47a81ce2be2426770693ffb7a155e54f9c556c40b943ce88d1859
        let expected_hash =
            felt!("0x1f01dd957c47a81ce2be2426770693ffb7a155e54f9c556c40b943ce88d1859");

        let sender_address =
            felt!("0x4d2c7d94a05cd95e08f1c135c53aa798f26ac383198d77bd37822e646cbab44");
        let class_hash = felt!("0xd0879f156c3e060638d5fb8ea1604cada1a29017988b3ee4f5f8b653279f60");
        let max_fee = felt!("0x1cfe57d53f9f");
        let chain_id = ChainId::MAINNET.id();
        let nonce = felt!("0xb");

        let actual_hash = compute_declare_v1_tx_hash(
            sender_address,
            class_hash,
            max_fee.to_u128().unwrap(),
            chain_id,
            nonce,
            false,
        );

        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_declare_v2_tx_hash() {
        // Starknet mainnet tx hash: https://voyager.online/tx/0x836d1c53ecf839a36e1e0bc3a3b9bb8087ca152313da63c3198773c8004cb9
        let expected_hash =
            felt!("0x836d1c53ecf839a36e1e0bc3a3b9bb8087ca152313da63c3198773c8004cb9");

        let sender_address =
            felt!("0x020c398d72af5efa4b63f5e3d5ad21e981d6af5f5929cfd2ab0d759ff935be53");
        let class_hash =
            felt!("0x0311b6f080fd3385e7154ca3a8568eb7d6aebcb7ff627c1f5e7d2cc99aeb7741");
        let max_fee = felt!("0x108ae97efa9f8");
        let chain_id = ChainId::MAINNET.id();
        let nonce = felt!("0xb");
        let compiled_class_hash =
            felt!("0x29b2702c06c1e3f3fe79e5b5e89071e9c4a8e82955a633a3879e3fae1dd7c3c");

        let actual_hash = compute_declare_v2_tx_hash(
            sender_address,
            class_hash,
            max_fee.to_u128().unwrap(),
            chain_id,
            nonce,
            compiled_class_hash,
            false,
        );

        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_declare_v3_tx_hash() {
        let expected_hash =
            felt!("0x41d1f5206ef58a443e7d3d1ca073171ec25fa75313394318fc83a074a6631c3");

        let sender_address =
            felt!("0x2fab82e4aef1d8664874e1f194951856d48463c3e6bf9a8c68e234a629a6f50");
        let class_hash = felt!("0x5ae9d09292a50ed48c5930904c880dab56e85b825022a7d689cfc9e65e01ee7");
        let compiled_class_hash =
            felt!("0x1add56d64bebf8140f3b8a38bdf102b7874437f0c861ab4ca7526ec33b4d0f8");
        let tip = 0;
        let l1_gas_bounds = ResourceBounds { max_amount: 0x186a0, max_price_per_unit: 0x2540be400 };
        let l2_gas_bounds = ResourceBounds { max_amount: 0x0, max_price_per_unit: 0x0 };
        let paymaster_data = vec![];
        let chain_id = ChainId::GOERLI.id();
        let nonce = felt!("0x1");
        let nonce_da_mode = &DataAvailabilityMode::L1;
        let fee_da_mode = &DataAvailabilityMode::L1;
        let account_deployment_data = vec![];

        let actual_hash = compute_declare_v3_tx_hash(
            sender_address,
            class_hash,
            compiled_class_hash,
            tip,
            &l1_gas_bounds,
            &l2_gas_bounds,
            &paymaster_data,
            chain_id,
            nonce,
            nonce_da_mode,
            fee_da_mode,
            &account_deployment_data,
            false,
        );

        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_invoke_v1_tx_hash() {
        // Starknet mainnet tx hash: https://voyager.online/tx/0x10a50b9fb1b23acadf2624d456578441fd00b94556928bf68478a2b3eabdfe8
        let expected_hash =
            felt!("0x10a50b9fb1b23acadf2624d456578441fd00b94556928bf68478a2b3eabdfe8");

        let sender_address =
            felt!("0x1e8b29765eb24b1cc13e21d9112e1ebebefa7cd5f1aee54be06dc19c831d22");
        let calldata = vec![
            felt!("0x2"),
            felt!("0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"),
            felt!("0x219209e083275171774dab1df80982e9df2096516f06319c5c6d71ae0a8480c"),
            felt!("0x0"),
            felt!("0x3"),
            felt!("0x41fd22b238fa21cfcf5dd45a8548974d8263b3a531a60388411c5e230f97023"),
            felt!("0x3276861cf5e05d6daf8f352cabb47df623eb10c383ab742fcc7abea94d5c5cc"),
            felt!("0x3"),
            felt!("0x9"),
            felt!("0xc"),
            felt!("0x41fd22b238fa21cfcf5dd45a8548974d8263b3a531a60388411c5e230f97023"),
            felt!("0x9184e72a000"),
            felt!("0x0"),
            felt!("0x9184e72a000"),
            felt!("0x0"),
            felt!("0x4634"),
            felt!("0x0"),
            felt!("0x2"),
            felt!("0x49d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"),
            felt!("0x53c91253bc9682c04929ca02ed00b3e423f6710d2ee7e0d5ebb06f3ecf368a8"),
            felt!("0x1e8b29765eb24b1cc13e21d9112e1ebebefa7cd5f1aee54be06dc19c831d22"),
            felt!("0x646d2c15"),
        ];
        let max_fee = felt!("0x113b8bbfd40de0");
        let chain_id = ChainId::MAINNET.id();
        let nonce = felt!("0x1");

        let actual_hash = compute_invoke_v1_tx_hash(
            sender_address,
            &calldata,
            max_fee.to_u128().unwrap(),
            chain_id,
            nonce,
            false,
        );

        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_invoke_v3_tx_hash() {
        // Starknet mainnet tx hash: https://voyager.online/tx/0x4750cd5a3cae0974215f0468bddb7df83c4209ae1fd0222d50c31980b1641d0
        let expected_hash =
            felt!("0x4750cd5a3cae0974215f0468bddb7df83c4209ae1fd0222d50c31980b1641d0");

        let sender_address =
            felt!("0x686735619287df0f11ec4cda22675f780886b52bf59cf899dd57fd5d5f4cad");
        let calldata = vec![
            felt!("0x1"),
            felt!("0x422d33a3638dcc4c62e72e1d6942cd31eb643ef596ccac2351e0e21f6cd4bf4"),
            felt!("0xcaffbd1bd76bd7f24a3fa1d69d1b2588a86d1f9d2359b13f6a84b7e1cbd126"),
            felt!("0x6"),
            felt!("0x436f6e737472756374696f6e4162616e646f6e"),
            felt!("0x4"),
            felt!("0x5"),
            felt!("0x37ee"),
            felt!("0x1"),
            felt!("0xcdd"),
        ];
        let tip = 0;
        let l1_gas_bounds = ResourceBounds { max_amount: 0x9b, max_price_per_unit: 0x1d744c7328c8 };
        let l2_gas_bounds = ResourceBounds { max_amount: 0x0, max_price_per_unit: 0x0 };
        let paymaster_data = vec![];
        let chain_id = ChainId::MAINNET.id();
        let nonce = felt!("0x761");
        let nonce_da_mode = &DataAvailabilityMode::L1;
        let fee_da_mode = &DataAvailabilityMode::L1;
        let account_deployment_data = vec![];

        let actual_hash = compute_invoke_v3_tx_hash(
            sender_address,
            &calldata,
            tip,
            &l1_gas_bounds,
            &l2_gas_bounds,
            &paymaster_data,
            chain_id,
            nonce,
            nonce_da_mode,
            fee_da_mode,
            &account_deployment_data,
            false,
        );

        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn test_compute_l1_handler_tx_hash() {
        // Starknet mainnet tx hash: https://voyager.online/tx/0x30d300980374bd923b0d0848ef18c41c071439c5dce578755fb47bcc9b9708b
        let expected_hash =
            felt!("0x30d300980374bd923b0d0848ef18c41c071439c5dce578755fb47bcc9b9708b");

        let version = felt!("0x0");
        let contract_address =
            felt!("0x73314940630fd6dcda0d772d4c972c4e0a9946bef9dabf4ef84eda8ef542b82");
        let entry_point_selector =
            felt!("0x1b64b1b3b690b43b9b514fb81377518f4039cd3e4f4914d8a6bdf01d679fb19");
        let calldata = vec![
            felt!("0xae0ee0a63a2ce6baeeffe56e7714fb4efe48d419"),
            felt!("0x455448"),
            felt!("0x57f3dfd2675615978808285b74d6188caae37007"),
            felt!("0x4229875ec8b6ad7490cc79e47fda6c4839172238fbd2978da9633305439d84d"),
            felt!("0x61b31ab352c0000"),
            felt!("0x0"),
        ];
        let chain_id = ChainId::MAINNET.id();
        let nonce = felt!("0x194cb1");

        let actual_hash = compute_l1_handler_tx_hash(
            version,
            contract_address,
            entry_point_selector,
            &calldata,
            chain_id,
            nonce,
        );

        assert_eq!(actual_hash, expected_hash);
    }
}
