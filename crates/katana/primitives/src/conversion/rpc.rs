use std::collections::HashMap;
use std::io::{Read, Write};

use anyhow::{anyhow, Result};
use blockifier::execution::contract_class::ContractClassV0;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_vm::serde::deserialize_program::ProgramJson;
use serde_json::json;
pub use starknet::core::types::contract::legacy::{LegacyContractClass, LegacyProgram};
pub use starknet::core::types::contract::CompiledClass;
use starknet::core::types::{
    BroadcastedDeclareTransaction, BroadcastedDeployAccountTransaction,
    BroadcastedInvokeTransaction, CompressedLegacyContractClass, ContractClass,
    LegacyContractEntryPoint, LegacyEntryPointsByType,
};
use starknet::core::utils::get_contract_address;
use starknet_api::deprecated_contract_class::{EntryPoint, EntryPointType};

use self::primitives::{ContractAddress, InvokeTxV1};
use crate::contract::{ClassHash, CompiledClassHash, CompiledContractClass, SierraClass};
use crate::utils::transaction::{
    compute_declare_v1_transaction_hash, compute_declare_v2_transaction_hash,
    compute_deploy_account_v1_transaction_hash, compute_invoke_v1_transaction_hash,
};
use crate::FieldElement;

mod primitives {
    pub use crate::contract::{CompiledContractClass, ContractAddress, Nonce};
    pub use crate::transaction::{
        DeclareTx, DeclareTxV1, DeclareTxV2, DeclareTxWithCompiledClass, DeployAccountTx,
        DeployAccountTxWithContractAddress, InvokeTx, InvokeTxV1, L1HandlerTx, Transaction,
    };
    pub use crate::FieldElement;
}

// Transactions

impl primitives::InvokeTx {
    pub fn from_broadcasted_rpc(tx: BroadcastedInvokeTransaction, chain_id: FieldElement) -> Self {
        let transaction_hash = compute_invoke_v1_transaction_hash(
            tx.sender_address,
            &tx.calldata,
            tx.max_fee,
            chain_id,
            tx.nonce,
            tx.is_query,
        );

        primitives::InvokeTx::V1(InvokeTxV1 {
            transaction_hash,
            nonce: tx.nonce,
            calldata: tx.calldata,
            signature: tx.signature,
            sender_address: tx.sender_address.into(),
            max_fee: tx.max_fee.try_into().expect("max_fee is too large"),
        })
    }
}

impl primitives::DeployAccountTx {
    pub fn from_broadcasted_rpc(
        tx: BroadcastedDeployAccountTransaction,
        chain_id: FieldElement,
    ) -> (Self, ContractAddress) {
        let contract_address = get_contract_address(
            tx.contract_address_salt,
            tx.class_hash,
            &tx.constructor_calldata,
            FieldElement::ZERO,
        );

        let transaction_hash = compute_deploy_account_v1_transaction_hash(
            contract_address,
            &tx.constructor_calldata,
            tx.class_hash,
            tx.contract_address_salt,
            tx.max_fee,
            chain_id,
            tx.nonce,
            tx.is_query,
        );

        (
            Self {
                transaction_hash,
                nonce: tx.nonce,
                signature: tx.signature,
                class_hash: tx.class_hash,
                version: FieldElement::ONE,
                constructor_calldata: tx.constructor_calldata,
                contract_address_salt: tx.contract_address_salt,
                max_fee: tx.max_fee.try_into().expect("max_fee is too large"),
            },
            contract_address.into(),
        )
    }
}

impl primitives::DeclareTx {
    pub fn from_broadcasted_rpc(
        tx: BroadcastedDeclareTransaction,
        chain_id: FieldElement,
    ) -> (Self, primitives::CompiledContractClass) {
        match tx {
            BroadcastedDeclareTransaction::V1(tx) => {
                let (class_hash, contract_class) =
                    legacy_rpc_to_inner_class(&tx.contract_class).expect("valid contract class");

                let transaction_hash = compute_declare_v1_transaction_hash(
                    tx.sender_address,
                    class_hash,
                    tx.max_fee,
                    chain_id,
                    tx.nonce,
                    tx.is_query,
                );

                (
                    primitives::DeclareTx::V1(primitives::DeclareTxV1 {
                        class_hash,
                        nonce: tx.nonce,
                        transaction_hash,
                        signature: tx.signature,
                        sender_address: tx.sender_address.into(),
                        max_fee: tx.max_fee.try_into().expect("max_fee is too large"),
                    }),
                    contract_class,
                )
            }

            BroadcastedDeclareTransaction::V2(tx) => {
                let (class_hash, _, contract_class) =
                    rpc_to_inner_class(&tx.contract_class).expect("valid contract class");

                let transaction_hash = compute_declare_v2_transaction_hash(
                    tx.sender_address,
                    class_hash,
                    tx.max_fee,
                    chain_id,
                    tx.nonce,
                    tx.compiled_class_hash,
                    tx.is_query,
                );

                (
                    primitives::DeclareTx::V2(primitives::DeclareTxV2 {
                        class_hash,
                        nonce: tx.nonce,
                        transaction_hash,
                        signature: tx.signature,
                        sender_address: tx.sender_address.into(),
                        compiled_class_hash: tx.compiled_class_hash,
                        max_fee: tx.max_fee.try_into().expect("max_fee is too large"),
                    }),
                    contract_class,
                )
            }
        }
    }
}

// Contract class

pub fn legacy_inner_to_rpc_class(legacy_contract_class: ContractClassV0) -> Result<ContractClass> {
    let entry_points_by_type =
        to_rpc_legacy_entry_points_by_type(&legacy_contract_class.entry_points_by_type)?;

    let program = {
        let program: ProgramJson = legacy_contract_class.program.clone().into();
        compress(&serde_json::to_vec(&program)?)?
    };

    Ok(ContractClass::Legacy(CompressedLegacyContractClass {
        program,
        abi: None,
        entry_points_by_type,
    }))
}

pub fn rpc_to_inner_class(
    contract_class: &SierraClass,
) -> Result<(ClassHash, CompiledClassHash, CompiledContractClass)> {
    let class_hash = contract_class.class_hash();

    let contract_class = rpc_to_cairo_contract_class(contract_class)?;
    let casm_contract = CasmContractClass::from_contract_class(contract_class, true)?;

    // compute compiled class hash
    let res = serde_json::to_string(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;

    Ok((
        class_hash,
        compiled_class.class_hash()?,
        CompiledContractClass::V1(casm_contract.try_into()?),
    ))
}

/// Converts `starknet-rs` RPC [SierraClass] type to Cairo's
/// [ContractClass](cairo_lang_starknet::contract_class::ContractClass) type.
pub fn rpc_to_cairo_contract_class(
    contract_class: &SierraClass,
) -> Result<cairo_lang_starknet::contract_class::ContractClass, std::io::Error> {
    let value = serde_json::to_value(contract_class)?;

    Ok(cairo_lang_starknet::contract_class::ContractClass {
        abi: serde_json::from_value(value["abi"].clone()).ok(),
        sierra_program: serde_json::from_value(value["sierra_program"].clone())?,
        entry_points_by_type: serde_json::from_value(value["entry_points_by_type"].clone())?,
        contract_class_version: serde_json::from_value(value["contract_class_version"].clone())?,
        sierra_program_debug_info: serde_json::from_value(
            value["sierra_program_debug_info"].clone(),
        )
        .ok(),
    })
}

/// Compute the compiled class hash from the given [FlattenedSierraClass].
pub fn compiled_class_hash_from_flattened_sierra_class(
    contract_class: &SierraClass,
) -> Result<FieldElement> {
    let contract_class = rpc_to_cairo_contract_class(contract_class)?;
    let casm_contract = CasmContractClass::from_contract_class(contract_class, true)?;
    let res = serde_json::to_string(&casm_contract)?;
    let compiled_class: CompiledClass = serde_json::from_str(&res)?;
    Ok(compiled_class.class_hash()?)
}

pub fn legacy_rpc_to_inner_class(
    compressed_legacy_contract: &CompressedLegacyContractClass,
) -> Result<(ClassHash, CompiledContractClass)> {
    let legacy_program_json = decompress(&compressed_legacy_contract.program)?;
    let legacy_program: LegacyProgram = serde_json::from_str(&legacy_program_json)?;

    let flattened = json!({
        "program": legacy_program,
        "abi": compressed_legacy_contract.abi,
        "entry_points_by_type": compressed_legacy_contract.entry_points_by_type,
    });

    let legacy_contract_class: LegacyContractClass = serde_json::from_value(flattened.clone())?;
    let class_hash = legacy_contract_class.class_hash()?;
    let contract_class: ContractClassV0 = serde_json::from_value(flattened)?;

    Ok((class_hash, CompiledContractClass::V0(contract_class)))
}

/// Returns a [LegacyEntryPointsByType](rpc::LegacyEntryPointsByType) (RPC type) from a
/// [EntryPointType] (blockifier type)
fn to_rpc_legacy_entry_points_by_type(
    entries: &HashMap<EntryPointType, Vec<EntryPoint>>,
) -> Result<LegacyEntryPointsByType> {
    fn collect_entry_points(
        entries: &HashMap<EntryPointType, Vec<EntryPoint>>,
        entry_point_type: &EntryPointType,
    ) -> Result<Vec<LegacyContractEntryPoint>> {
        Ok(entries
            .get(entry_point_type)
            .ok_or(anyhow!("Missing {entry_point_type:?} entry point",))?
            .iter()
            .map(|e| LegacyContractEntryPoint {
                offset: e.offset.0 as u64,
                selector: FieldElement::from(e.selector.0),
            })
            .collect::<Vec<_>>())
    }

    let constructor = collect_entry_points(entries, &EntryPointType::Constructor)?;
    let external = collect_entry_points(entries, &EntryPointType::External)?;
    let l1_handler = collect_entry_points(entries, &EntryPointType::L1Handler)?;

    Ok(LegacyEntryPointsByType { constructor, external, l1_handler })
}

fn compress(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut gzip_encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    Write::write_all(&mut gzip_encoder, data)?;
    gzip_encoder.finish()
}

fn decompress(data: &[u8]) -> Result<String, std::io::Error> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut decoded = String::new();
    Read::read_to_string(&mut decoder, &mut decoded)?;
    Ok(decoded)
}
