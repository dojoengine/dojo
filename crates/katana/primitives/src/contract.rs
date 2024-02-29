use std::fmt;

use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use derive_more::Deref;
use starknet::core::utils::normalize_address;

use crate::FieldElement;

/// Represents the type for a contract storage key.
pub type StorageKey = FieldElement;
/// Represents the type for a contract storage value.
pub type StorageValue = FieldElement;

/// The canonical hash of a contract class. This is the class hash value of a contract instance.
pub type ClassHash = FieldElement;
/// The hash of a compiled contract class.
pub type CompiledClassHash = FieldElement;

/// Represents the type for a contract nonce.
pub type Nonce = FieldElement;

pub type SierraClass = starknet::core::types::contract::SierraClass;
pub type FlattenedSierraClass = starknet::core::types::FlattenedSierraClass;

/// Represents a contract address.
#[derive(Default, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Deref)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContractAddress(#[deref] pub FieldElement);

impl ContractAddress {
    pub fn new(address: FieldElement) -> Self {
        ContractAddress(normalize_address(address))
    }
}

impl fmt::Display for ContractAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl From<FieldElement> for ContractAddress {
    fn from(value: FieldElement) -> Self {
        ContractAddress::new(value)
    }
}

impl From<ContractAddress> for FieldElement {
    fn from(value: ContractAddress) -> Self {
        value.0
    }
}

/// Represents a generic contract instance information.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GenericContractInfo {
    /// The nonce of the contract.
    pub nonce: Nonce,
    /// The hash of the contract class.
    pub class_hash: ClassHash,
}

pub type DeprecatedCompiledClass = ::starknet_api::deprecated_contract_class::ContractClass;

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SierraProgram {
    pub program: cairo_lang_sierra::program::Program,
    pub entry_points_by_type: cairo_lang_starknet::contract_class::ContractEntryPoints,
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
    Deprecated(DeprecatedCompiledClass),
    Class(SierraCompiledClass),
}
