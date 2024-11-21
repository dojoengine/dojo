use anyhow::Result;
use katana_cairo::lang::starknet_classes::casm_contract_class::CasmContractClass;
use katana_cairo::lang::starknet_classes::contract_class::ContractClass;
use serde_json::Value;

use crate::class::{CompiledClass, LegacyContractClass, SierraClass};

// TODO: this was taken from the current network limit
// https://docs.starknet.io/documentation/tools/limits_and_triggers/.
// We may want to make this configurable.
// We also need this value into `dojo-world`.. which is not ideal as we don't want
// primitives to depend on `dojo-world` or vice-versa.
// pub const MAX_BYTECODE_SIZE: usize = 81_290;

pub fn parse_compiled_class(artifact: Value) -> Result<CompiledClass> {
    if let Ok(casm) = parse_compiled_class_v1(artifact.clone()) {
        Ok(CompiledClass::Class(casm))
    } else {
        Ok(CompiledClass::Legacy(parse_deprecated_compiled_class(artifact)?))
    }
}

pub fn parse_compiled_class_v1(class: Value) -> Result<CasmContractClass> {
    let class: ContractClass = serde_json::from_value(class)?;
    Ok(CasmContractClass::from_contract_class(class, true, usize::MAX)?)
}

/// Parse a [`str`] into a [`SierraClass`].
pub fn parse_sierra_class(class: &str) -> Result<SierraClass, serde_json::Error> {
    serde_json::from_str(class)
}

pub fn parse_deprecated_compiled_class(
    class: Value,
) -> Result<LegacyContractClass, serde_json::Error> {
    serde_json::from_value(class)
}
