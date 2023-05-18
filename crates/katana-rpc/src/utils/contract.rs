use anyhow::{Context, Ok, Result};
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use starknet::core::types::contract::legacy::LegacyContractClass;
use starknet::core::types::contract::{CompiledClass, SierraClass};
use starknet::core::types::FieldElement;

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
