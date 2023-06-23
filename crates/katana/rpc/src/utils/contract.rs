use std::io::Read;

use anyhow::{Context, Ok, Result};
use blockifier::execution::contract_class::{ContractClass, ContractClassV0};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use serde_json::json;
use starknet::core::types::contract::legacy::{LegacyContractClass, LegacyProgram};
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::{CompressedLegacyContractClass, FieldElement, FlattenedSierraClass};

#[allow(unused)]
pub fn get_casm_class_hash(raw_contract_class: &str) -> Result<FieldElement> {
    let casm_contract_class: cairo_lang_starknet::contract_class::ContractClass =
        serde_json::from_str(raw_contract_class)
            .with_context(|| "unable to deserialize contract")?;
    let casm_contract = CasmContractClass::from_contract_class(casm_contract_class, true)
        .with_context(|| "unable to convert as CasmContractClass")?;
    let res = serde_json::to_string(&casm_contract)?;
    let compiled_class: CompiledClass =
        serde_json::from_str(&res).with_context(|| "unable to parse as CompiledClass")?;
    Ok(compiled_class.class_hash()?)
}

#[allow(unused)]
pub fn get_sierra_class_hash(raw_contract_class: &str) -> Result<FieldElement> {
    let sierra_class: SierraClass = serde_json::from_str(raw_contract_class)?;
    Ok(sierra_class.class_hash()?)
}

#[allow(unused)]
pub fn get_legacy_contract_class_hash(raw_contract_class: &str) -> Result<FieldElement> {
    let legacy_contract_class: LegacyContractClass = serde_json::from_str(raw_contract_class)?;
    Ok(legacy_contract_class.class_hash()?)
}

pub fn rpc_to_inner_class(
    contract_class: &FlattenedSierraClass,
) -> Result<(FieldElement, ContractClass)> {
    let class_hash = contract_class.class_hash();

    let value = serde_json::to_value(contract_class)?;
    let contract_class = cairo_lang_starknet::contract_class::ContractClass {
        abi: serde_json::from_value(value["abi"].clone()).ok(),
        sierra_program: serde_json::from_value(value["sierra_program"].clone())?,
        entry_points_by_type: serde_json::from_value(value["entry_points_by_type"].clone())?,
        contract_class_version: serde_json::from_value(value["contract_class_version"].clone())?,
        sierra_program_debug_info: serde_json::from_value(
            value["sierra_program_debug_info"].clone(),
        )
        .ok(),
    };

    let casm_contract = CasmContractClass::from_contract_class(contract_class, true)?;
    Ok((class_hash, ContractClass::V1(casm_contract.try_into()?)))
}

pub fn legacy_rpc_to_inner_class(
    compressed_legacy_contract: &CompressedLegacyContractClass,
) -> Result<(FieldElement, ContractClass)> {
    let legacy_program_json = decompress(&compressed_legacy_contract.program)?;
    let legacy_program: LegacyProgram = serde_json::from_str(&legacy_program_json)?;

    let flattened = json!({
        "program": legacy_program,
        "abi": compressed_legacy_contract.abi,
        "entry_points_by_type": compressed_legacy_contract.entry_points_by_type,
    });

    let legacy_contract_class: LegacyContractClass = serde_json::from_value(flattened.clone())?;
    let class_hash = legacy_contract_class.class_hash()?;
    let contract_class = serde_json::from_value::<ContractClassV0>(flattened)?;

    Ok((class_hash, ContractClass::V0(contract_class)))
}

fn decompress(data: &[u8]) -> Result<String> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut decoded = String::new();
    decoder.read_to_string(&mut decoded)?;
    Ok(decoded)
}
