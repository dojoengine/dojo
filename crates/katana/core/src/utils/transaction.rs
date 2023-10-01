use std::sync::Arc;

use blockifier::execution::contract_class::ContractClass;
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::transaction_execution::Transaction as ExecutionTransaction;
use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::types::{
    BroadcastedDeclareTransaction, BroadcastedDeployAccountTransaction,
    BroadcastedInvokeTransaction, DeclareTransaction, DeclareTransactionV1, DeclareTransactionV2,
    DeployAccountTransaction, DeployTransaction, FieldElement, InvokeTransaction,
    InvokeTransactionV0, InvokeTransactionV1, L1HandlerTransaction, Transaction as RpcTransaction,
};
use starknet::core::utils::{get_contract_address, parse_cairo_short_string};
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::{StarkFelt, StarkHash};
use starknet_api::transaction::{
    Calldata, ContractAddressSalt, DeclareTransaction as DeclareApiTransaction,
    DeclareTransactionV0V1 as DeclareApiTransactionV0V1,
    DeclareTransactionV2 as DeclareApiTransactionV2,
    DeployAccountTransaction as DeployAccountApiTransaction,
    DeployTransaction as DeployApiTransaction, Fee, InvokeTransaction as InvokeApiTransaction,
    InvokeTransactionV1 as InvokeApiTransactionV1, L1HandlerTransaction as L1HandlerApiTransaction,
    Transaction as ApiTransaction, TransactionHash, TransactionSignature, TransactionVersion,
};
use starknet_api::{patricia_key, stark_felt};

use super::contract::rpc_to_inner_class;
use crate::utils::contract::legacy_rpc_to_inner_class;
use crate::utils::starkfelt_to_u128;

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

/// Compute the hash of a V0 Invoke transaction.
pub fn compute_invoke_v0_transaction_hash(
    contract_address: FieldElement,
    entry_point_selector: FieldElement,
    calldata: &[FieldElement],
    max_fee: FieldElement,
    chain_id: FieldElement,
    is_query: bool,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_INVOKE,
        if is_query { QUERY_VERSION_OFFSET + FieldElement::ZERO } else { FieldElement::ZERO }, /* version */
        contract_address,
        entry_point_selector, // entry_point_selector
        compute_hash_on_elements(calldata),
        max_fee,
        chain_id,
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
/// from `L1HandlerApiTransaction`.
pub fn compute_l1_handler_transaction_hash(
    tx: L1HandlerApiTransaction,
    chain_id: FieldElement,
) -> FieldElement {
    let tx = api_l1_handler_to_rpc_transaction(tx);
    let version: FieldElement = tx.version.into();

    assert_eq!(version, FieldElement::ZERO, "L1 handler transaction only supports version 0");

    compute_l1_handler_transaction_hash_felts(
        version,
        tx.contract_address,
        tx.entry_point_selector,
        &tx.calldata,
        chain_id,
        tx.nonce.into(),
    )
}

/// Computes the hash of a L1 handler transaction
/// from the fields involved in the computation,
/// as felts values.
pub fn compute_l1_handler_transaction_hash_felts(
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

/// Convert [StarkFelt] array to [FieldElement] array.
#[inline]
pub fn stark_felt_to_field_element_array(arr: &[StarkFelt]) -> Vec<FieldElement> {
    arr.iter().map(|e| (*e).into()).collect()
}

/// Convert [starknet_api::transaction::Transaction] transaction to JSON-RPC compatible transaction,
/// [starknet::core::types::Transaction].
/// `starknet_api` transaction types are used when executing the transaction using `blockifier`.
pub fn api_to_rpc_transaction(transaction: ApiTransaction) -> RpcTransaction {
    match transaction {
        ApiTransaction::Invoke(invoke) => {
            RpcTransaction::Invoke(api_invoke_to_rpc_transaction(invoke))
        }
        ApiTransaction::Declare(declare) => {
            RpcTransaction::Declare(api_declare_to_rpc_transaction(declare))
        }
        ApiTransaction::DeployAccount(deploy) => {
            RpcTransaction::DeployAccount(api_deploy_account_to_rpc_transaction(deploy))
        }
        ApiTransaction::L1Handler(l1handler) => {
            RpcTransaction::L1Handler(api_l1_handler_to_rpc_transaction(l1handler))
        }
        ApiTransaction::Deploy(deploy) => {
            RpcTransaction::Deploy(api_deploy_to_rpc_transaction(deploy))
        }
    }
}

fn api_l1_handler_to_rpc_transaction(transaction: L1HandlerApiTransaction) -> L1HandlerTransaction {
    L1HandlerTransaction {
        transaction_hash: transaction.transaction_hash.0.into(),
        contract_address: (*transaction.contract_address.0.key()).into(),
        nonce: <StarkFelt as Into<FieldElement>>::into(transaction.nonce.0)
            .try_into()
            .expect("able to convert starkfelt to u64"),
        version: <StarkFelt as Into<FieldElement>>::into(transaction.version.0)
            .try_into()
            .expect("able to convert starkfelt to u64"),
        entry_point_selector: transaction.entry_point_selector.0.into(),
        calldata: stark_felt_to_field_element_array(&transaction.calldata.0),
    }
}

fn api_deploy_to_rpc_transaction(transaction: DeployApiTransaction) -> DeployTransaction {
    DeployTransaction {
        transaction_hash: transaction.transaction_hash.0.into(),
        version: <StarkFelt as Into<FieldElement>>::into(transaction.version.0)
            .try_into()
            .expect("able to convert starkfelt to u64"),
        class_hash: transaction.class_hash.0.into(),
        contract_address_salt: transaction.contract_address_salt.0.into(),
        constructor_calldata: stark_felt_to_field_element_array(
            &transaction.constructor_calldata.0,
        ),
    }
}

fn api_deploy_account_to_rpc_transaction(
    transaction: DeployAccountApiTransaction,
) -> DeployAccountTransaction {
    DeployAccountTransaction {
        nonce: transaction.nonce.0.into(),
        max_fee: transaction.max_fee.0.into(),
        class_hash: transaction.class_hash.0.into(),
        transaction_hash: transaction.transaction_hash.0.into(),
        contract_address_salt: transaction.contract_address_salt.0.into(),
        constructor_calldata: stark_felt_to_field_element_array(
            &transaction.constructor_calldata.0,
        ),
        signature: stark_felt_to_field_element_array(&transaction.signature.0),
    }
}

fn api_invoke_to_rpc_transaction(transaction: InvokeApiTransaction) -> InvokeTransaction {
    match transaction {
        InvokeApiTransaction::V0(tx) => InvokeTransaction::V0(InvokeTransactionV0 {
            max_fee: tx.max_fee.0.into(),
            transaction_hash: tx.transaction_hash.0.into(),
            contract_address: (*tx.contract_address.0.key()).into(),
            entry_point_selector: tx.entry_point_selector.0.into(),
            calldata: stark_felt_to_field_element_array(&tx.calldata.0),
            signature: stark_felt_to_field_element_array(&tx.signature.0),
        }),
        InvokeApiTransaction::V1(tx) => InvokeTransaction::V1(InvokeTransactionV1 {
            nonce: tx.nonce.0.into(),
            max_fee: tx.max_fee.0.into(),
            transaction_hash: tx.transaction_hash.0.into(),
            sender_address: (*tx.sender_address.0.key()).into(),
            calldata: stark_felt_to_field_element_array(&tx.calldata.0),
            signature: stark_felt_to_field_element_array(&tx.signature.0),
        }),
    }
}

fn api_declare_to_rpc_transaction(transaction: DeclareApiTransaction) -> DeclareTransaction {
    match transaction {
        DeclareApiTransaction::V0(tx) | DeclareApiTransaction::V1(tx) => {
            DeclareTransaction::V1(DeclareTransactionV1 {
                nonce: tx.nonce.0.into(),
                max_fee: tx.max_fee.0.into(),
                class_hash: tx.class_hash.0.into(),
                transaction_hash: tx.transaction_hash.0.into(),
                sender_address: (*tx.sender_address.0.key()).into(),
                signature: stark_felt_to_field_element_array(&tx.signature.0),
            })
        }
        DeclareApiTransaction::V2(tx) => DeclareTransaction::V2(DeclareTransactionV2 {
            nonce: tx.nonce.0.into(),
            max_fee: tx.max_fee.0.into(),
            class_hash: tx.class_hash.0.into(),
            transaction_hash: tx.transaction_hash.0.into(),
            sender_address: (*tx.sender_address.0.key()).into(),
            compiled_class_hash: tx.compiled_class_hash.0.into(),
            signature: stark_felt_to_field_element_array(&tx.signature.0),
        }),
    }
}

/// Convert `blockfiier` transaction type to `starknet_api` transaction.
pub fn convert_blockifier_to_api_tx(transaction: &ExecutionTransaction) -> ApiTransaction {
    match transaction {
        ExecutionTransaction::AccountTransaction(tx) => match tx {
            AccountTransaction::Invoke(tx) => ApiTransaction::Invoke(tx.clone()),
            AccountTransaction::Declare(tx) => ApiTransaction::Declare(tx.tx().clone()),
            AccountTransaction::DeployAccount(tx) => ApiTransaction::DeployAccount(tx.tx.clone()),
        },
        ExecutionTransaction::L1HandlerTransaction(tx) => ApiTransaction::L1Handler(tx.tx.clone()),
    }
}

/// Convert broadcasted Invoke transaction type from `starknet-rs` to `starknet_api`'s
/// Invoke transaction.
pub fn broadcasted_invoke_rpc_to_api_transaction(
    transaction: BroadcastedInvokeTransaction,
    chain_id: FieldElement,
) -> InvokeApiTransaction {
    let BroadcastedInvokeTransaction {
        calldata, max_fee, nonce, sender_address, signature, ..
    } = transaction;

    let hash = compute_invoke_v1_transaction_hash(
        sender_address,
        &calldata,
        max_fee,
        chain_id,
        nonce,
        transaction.is_query,
    );

    let transaction = InvokeApiTransactionV1 {
        nonce: Nonce(nonce.into()),
        transaction_hash: TransactionHash(hash.into()),
        sender_address: ContractAddress(patricia_key!(sender_address)),
        signature: TransactionSignature(signature.into_iter().map(|e| e.into()).collect()),
        calldata: Calldata(Arc::new(calldata.into_iter().map(|c| c.into()).collect())),
        max_fee: Fee(starkfelt_to_u128(max_fee.into()).expect("convert max fee StarkFelt to u128")),
    };

    InvokeApiTransaction::V1(transaction)
}

/// Convert broadcasted Declare transaction type from `starknet-rs` to `starknet_api`'s
/// Declare transaction.
///
/// Returns the transaction and the contract class.
pub fn broadcasted_declare_rpc_to_api_transaction(
    transaction: BroadcastedDeclareTransaction,
    chain_id: FieldElement,
) -> Result<(DeclareApiTransaction, ContractClass), Box<dyn std::error::Error>> {
    match transaction {
        BroadcastedDeclareTransaction::V1(tx) => {
            let (class_hash, contract) = legacy_rpc_to_inner_class(&tx.contract_class)?;

            let transaction_hash = compute_declare_v1_transaction_hash(
                tx.sender_address,
                class_hash,
                tx.max_fee,
                chain_id,
                tx.nonce,
                tx.is_query,
            );

            let transaction = DeclareApiTransactionV0V1 {
                nonce: Nonce(tx.nonce.into()),
                class_hash: ClassHash(class_hash.into()),
                transaction_hash: TransactionHash(transaction_hash.into()),
                sender_address: ContractAddress(patricia_key!(tx.sender_address)),
                max_fee: Fee(starkfelt_to_u128(tx.max_fee.into())
                    .expect("convert max fee StarkFelt to u128")),
                signature: TransactionSignature(
                    tx.signature.into_iter().map(|e| e.into()).collect(),
                ),
            };

            Ok((DeclareApiTransaction::V1(transaction), contract))
        }

        BroadcastedDeclareTransaction::V2(tx) => {
            let (class_hash, contract_class) = rpc_to_inner_class(&tx.contract_class)?;

            let transaction_hash = compute_declare_v2_transaction_hash(
                tx.sender_address,
                class_hash,
                tx.max_fee,
                chain_id,
                tx.nonce,
                tx.compiled_class_hash,
                tx.is_query,
            );

            let transaction = DeclareApiTransactionV2 {
                nonce: Nonce(tx.nonce.into()),
                class_hash: ClassHash(class_hash.into()),
                transaction_hash: TransactionHash(transaction_hash.into()),
                sender_address: ContractAddress(patricia_key!(tx.sender_address)),
                compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
                max_fee: Fee(starkfelt_to_u128(tx.max_fee.into())
                    .expect("convert max fee StarkFelt to u128")),
                signature: TransactionSignature(
                    tx.signature.into_iter().map(|e| e.into()).collect(),
                ),
            };

            Ok((DeclareApiTransaction::V2(transaction), contract_class))
        }
    }
}

/// Convert broadcasted DeployAccount transaction type from `starknet-rs` to `starknet_api`'s
/// DeployAccount transaction.
///
/// Returns the transaction and the contract address of the account to be deployed.
pub fn broadcasted_deploy_account_rpc_to_api_transaction(
    transaction: BroadcastedDeployAccountTransaction,
    chain_id: FieldElement,
) -> (DeployAccountApiTransaction, FieldElement) {
    let BroadcastedDeployAccountTransaction {
        nonce,
        max_fee,
        signature,
        class_hash,
        constructor_calldata,
        contract_address_salt,
        ..
    } = transaction;

    let contract_address = get_contract_address(
        contract_address_salt,
        class_hash,
        &constructor_calldata,
        FieldElement::ZERO,
    );

    let transaction_hash = compute_deploy_account_v1_transaction_hash(
        contract_address,
        &constructor_calldata,
        class_hash,
        contract_address_salt,
        max_fee,
        chain_id,
        nonce,
        transaction.is_query,
    );

    let api_transaction = DeployAccountApiTransaction {
        signature: TransactionSignature(signature.into_iter().map(|s| s.into()).collect()),
        contract_address_salt: ContractAddressSalt(StarkFelt::from(contract_address_salt)),
        constructor_calldata: Calldata(Arc::new(
            constructor_calldata.into_iter().map(|d| d.into()).collect(),
        )),
        class_hash: ClassHash(class_hash.into()),
        max_fee: Fee(starkfelt_to_u128(max_fee.into()).expect("convert max fee StarkFelt to u128")),
        nonce: Nonce(nonce.into()),
        transaction_hash: TransactionHash(transaction_hash.into()),
        version: TransactionVersion(stark_felt!(1_u32)),
    };

    (api_transaction, contract_address)
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

        let hash = compute_deploy_account_v1_transaction_hash(
            contract_address,
            &constructor_calldata,
            class_hash,
            salt,
            max_fee,
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

pub fn warn_message_transaction_error_exec_error(err: &TransactionExecutionError) {
    match err {
        TransactionExecutionError::EntryPointExecutionError(ref eperr)
        | TransactionExecutionError::ExecutionError(ref eperr) => match eperr {
            EntryPointExecutionError::ExecutionFailed { error_data } => {
                let mut reasons: Vec<String> = vec![];
                error_data.iter().for_each(|felt| {
                    if let Ok(s) = parse_cairo_short_string(&FieldElement::from(*felt)) {
                        reasons.push(s);
                    }
                });

                tracing::warn!(target: "executor",
                               "Transaction validation error: {}", reasons.join(" "));
            }
            _ => tracing::warn!(target: "executor",
                                "Transaction validation error: {:?}", err),
        },
        _ => tracing::warn!(target: "executor",
                            "Transaction validation error: {:?}", err),
    }
}
