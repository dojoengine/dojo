use katana_cairo::lang::sierra::program::Program;
use katana_cairo::lang::starknet_classes::casm_contract_class::CasmContractClass;
use katana_cairo::lang::starknet_classes::contract_class::ContractEntryPoints;

use crate::Felt;

/// The canonical hash of a contract class. This is the identifier of a class.
pub type ClassHash = Felt;
/// The hash of a compiled contract class.
pub type CompiledClassHash = Felt;

pub type SierraClass = starknet::core::types::contract::SierraClass;
pub type FlattenedSierraClass = starknet::core::types::FlattenedSierraClass;

/// Deprecated legacy (Cairo 0) CASM class
pub type DeprecatedCompiledClass =
    ::katana_cairo::starknet_api::deprecated_contract_class::ContractClass;

/// Represents an executable Sierra program.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SierraProgram {
    pub program: Program,
    pub entry_points_by_type: ContractEntryPoints,
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SierraCompiledClass {
    pub casm: CasmContractClass,
    pub sierra: SierraProgram,
}

/// Executable contract class
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Eq, PartialEq, derive_more::From)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CompiledClass {
    Class(SierraCompiledClass),
    Deprecated(DeprecatedCompiledClass),
}
