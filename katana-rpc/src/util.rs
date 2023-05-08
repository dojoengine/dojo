use std::str::FromStr;

use anyhow::{Context, Ok, Result};
use blockifier::execution::contract_class::{
    casm_contract_into_contract_class, ContractClass as BlockifierContractClass,
};
use cairo_lang_starknet::{casm_contract_class::CasmContractClass, contract_class::ContractClass};
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::{CompiledClass, FlattenedSierraClass, SierraClass};
use starknet::{
    core::{crypto::compute_hash_on_elements, types::FieldElement},
    providers::jsonrpc::models::{
        DeclareTransaction, DeclareTransactionV1, DeclareTransactionV2, DeployAccountTransaction,
        InvokeTransaction, InvokeTransactionV1, L1HandlerTransaction, Transaction,
    },
};
use starknet_api::{
    hash::StarkFelt,
    transaction::{
        DeclareTransaction as InnerDeclareTransaction,
        DeployAccountTransaction as InnerDeployAccountTransaction,
        InvokeTransaction as InnerInvokeTransaction,
        L1HandlerTransaction as InnerL1HandlerTransaction, Transaction as InnerTransaction,
    },
};

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

pub fn compute_declare_v1_transaction_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    max_fee: FieldElement,
    chain_id: FieldElement,
    nonce: FieldElement,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        FieldElement::ONE, // version
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&[class_hash]),
        max_fee,
        chain_id,
        nonce,
    ])
}

pub fn compute_declare_v2_transaction_hash(
    sender_address: FieldElement,
    class_hash: FieldElement,
    max_fee: FieldElement,
    chain_id: FieldElement,
    nonce: FieldElement,
    compiled_class_hash: FieldElement,
) -> FieldElement {
    compute_hash_on_elements(&[
        PREFIX_DECLARE,
        FieldElement::TWO, // version
        sender_address,
        FieldElement::ZERO, // entry_point_selector
        compute_hash_on_elements(&[class_hash]),
        max_fee,
        chain_id,
        nonce,
        compiled_class_hash,
    ])
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

pub fn convert_stark_felt_array_to_field_element_array(
    calldata: &[StarkFelt],
) -> Result<Vec<FieldElement>> {
    calldata.iter().try_fold(Vec::new(), |mut data, &felt| {
        data.push(stark_felt_to_field_element(felt)?);
        Ok(data)
    })
}

pub fn convert_inner_to_rpc_tx(transaction: InnerTransaction) -> Result<Transaction> {
    let tx = match transaction {
        InnerTransaction::Invoke(invoke) => Transaction::Invoke(convert_invoke_to_rpc_tx(invoke)?),
        InnerTransaction::Declare(declare) => {
            Transaction::Declare(convert_declare_to_rpc_tx(declare)?)
        }
        InnerTransaction::DeployAccount(deploy) => {
            Transaction::DeployAccount(convert_deploy_account_to_rpc_tx(deploy)?)
        }
        InnerTransaction::L1Handler(l1handler) => {
            Transaction::L1Handler(convert_l1_handle_to_rpc(l1handler)?)
        }
        InnerTransaction::Deploy(_) => unimplemented!("deploy transaction not supported"),
    };
    Ok(tx)
}

fn convert_l1_handle_to_rpc(
    transaction: InnerL1HandlerTransaction,
) -> Result<L1HandlerTransaction> {
    Ok(L1HandlerTransaction {
        transaction_hash: stark_felt_to_field_element(transaction.transaction_hash.0)?,
        contract_address: stark_felt_to_field_element(*transaction.contract_address.0.key())?,
        nonce: stark_felt_to_field_element(transaction.nonce.0)?
            .try_into()
            .unwrap(),
        version: stark_felt_to_field_element(transaction.version.0)?
            .try_into()
            .unwrap(),
        entry_point_selector: stark_felt_to_field_element(transaction.entry_point_selector.0)?,
        calldata: convert_stark_felt_array_to_field_element_array(&transaction.calldata.0)?,
    })
}

fn convert_deploy_account_to_rpc_tx(
    transaction: InnerDeployAccountTransaction,
) -> Result<DeployAccountTransaction> {
    Ok(DeployAccountTransaction {
        transaction_hash: stark_felt_to_field_element(transaction.transaction_hash.0)?,
        version: stark_felt_to_field_element(transaction.version.0)?.try_into()?,
        class_hash: stark_felt_to_field_element(transaction.class_hash.0)?,
        contract_address_salt: stark_felt_to_field_element(transaction.contract_address_salt.0)?,
        nonce: stark_felt_to_field_element(transaction.nonce.0)?,
        constructor_calldata: convert_stark_felt_array_to_field_element_array(
            &transaction.constructor_calldata.0,
        )?,
        signature: convert_stark_felt_array_to_field_element_array(&transaction.signature.0)?,
        max_fee: FieldElement::from_str(&transaction.max_fee.0.to_string())?,
    })
}

fn convert_invoke_to_rpc_tx(transaction: InnerInvokeTransaction) -> Result<InvokeTransaction> {
    Ok(match transaction {
        InnerInvokeTransaction::V1(tx) => InvokeTransaction::V1(InvokeTransactionV1 {
            transaction_hash: stark_felt_to_field_element(tx.transaction_hash.0)?,
            sender_address: stark_felt_to_field_element(*tx.sender_address.0.key())?,
            nonce: stark_felt_to_field_element(tx.nonce.0)?,
            calldata: convert_stark_felt_array_to_field_element_array(&tx.calldata.0)?,
            signature: convert_stark_felt_array_to_field_element_array(&tx.signature.0)?,
            max_fee: FieldElement::from_str(&tx.max_fee.0.to_string())?,
        }),
        _ => unimplemented!("invoke v0 not supported"),
    })
}

fn convert_declare_to_rpc_tx(transaction: InnerDeclareTransaction) -> Result<DeclareTransaction> {
    Ok(match transaction {
        InnerDeclareTransaction::V0(tx) | InnerDeclareTransaction::V1(tx) => {
            DeclareTransaction::V1(DeclareTransactionV1 {
                nonce: stark_felt_to_field_element(tx.nonce.0)?,
                max_fee: FieldElement::from_str(&tx.max_fee.0.to_string())?,
                class_hash: stark_felt_to_field_element(tx.class_hash.0)?,
                transaction_hash: stark_felt_to_field_element(tx.transaction_hash.0)?,
                sender_address: stark_felt_to_field_element(*tx.sender_address.0.key())?,
                signature: convert_stark_felt_array_to_field_element_array(&tx.signature.0)?,
            })
        }
        InnerDeclareTransaction::V2(tx) => DeclareTransaction::V2(DeclareTransactionV2 {
            nonce: stark_felt_to_field_element(tx.nonce.0)?,
            max_fee: FieldElement::from_str(&tx.max_fee.0.to_string())?,
            class_hash: stark_felt_to_field_element(tx.class_hash.0)?,
            transaction_hash: stark_felt_to_field_element(tx.transaction_hash.0)?,
            sender_address: stark_felt_to_field_element(*tx.sender_address.0.key())?,
            signature: convert_stark_felt_array_to_field_element_array(&tx.signature.0)?,
            compiled_class_hash: stark_felt_to_field_element(tx.compiled_class_hash.0)?,
        }),
    })
}

pub fn get_casm_class_hash(raw_contract_class: &str) -> Result<FieldElement> {
    let casm_contract_class: ContractClass = serde_json::from_str(raw_contract_class)
        .with_context(|| "unable to deserialize contract")?;
    let casm_contract = CasmContractClass::from_contract_class(casm_contract_class, true)
        .with_context(|| "unable to convert as CasmContractClass")?;
    let res = serde_json::to_string(&casm_contract)?;
    let compiled_class: CompiledClass =
        serde_json::from_str(&res).with_context(|| "unable to parse as CompiledClass")?;
    Ok(compiled_class.class_hash()?)
}

pub fn get_sierra_class_hash(raw_contract_class: &str) -> Result<FieldElement> {
    let sierra_class: SierraClass = serde_json::from_str(raw_contract_class)?;
    Ok(sierra_class.class_hash()?)
}

pub fn get_legacy_contract_class_hash(raw_contract_class: &str) -> Result<FieldElement> {
    let legacy_contract_class: LegacyContractClass = serde_json::from_str(raw_contract_class)?;
    Ok(legacy_contract_class.class_hash()?)
}

pub fn get_casm_contract_class(raw_contract_class: &str) -> Result<BlockifierContractClass> {
    let casm_contract_class: ContractClass = serde_json::from_str(raw_contract_class)?;
    let casm_contract = CasmContractClass::from_contract_class(casm_contract_class, true)?;
    Ok(casm_contract_into_contract_class(casm_contract)?)
}

pub fn get_flattened_sierra_class(raw_contract_class: &str) -> Result<FlattenedSierraClass> {
    let contract_artifact: SierraClass = serde_json::from_str(raw_contract_class)?;
    Ok(contract_artifact.flatten()?)
}
