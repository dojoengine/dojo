use anyhow::Result;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use cairo_lang_starknet::contract_class::ContractClass;
use serde_json::Value;

use crate::class::{
    CompiledClass, DeprecatedCompiledClass, SierraClass, SierraCompiledClass, SierraProgram,
};

pub fn parse_compiled_class(artifact: Value) -> Result<CompiledClass> {
    if let Ok(class) = parse_compiled_class_v1(artifact.clone()) {
        Ok(CompiledClass::Class(class))
    } else {
        Ok(CompiledClass::Deprecated(parse_deprecated_compiled_class(artifact)?))
    }
}

pub fn parse_compiled_class_v1(class: Value) -> Result<SierraCompiledClass> {
    let class: ContractClass = serde_json::from_value(class)?;

    let program = class.extract_sierra_program()?;
    let entry_points_by_type = class.entry_points_by_type.clone();
    let sierra = SierraProgram { program, entry_points_by_type };

    let casm = CasmContractClass::from_contract_class(class, true)?;

    Ok(SierraCompiledClass { casm, sierra })
}

/// Parse a [`str`] into a [`SierraClass`].
pub fn parse_sierra_class(class: &str) -> Result<SierraClass, serde_json::Error> {
    serde_json::from_str(class)
}

pub fn parse_deprecated_compiled_class(
    class: Value,
) -> Result<DeprecatedCompiledClass, serde_json::Error> {
    serde_json::from_value(class)
}
