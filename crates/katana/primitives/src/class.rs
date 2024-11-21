use katana_cairo::lang::starknet_classes::abi;
use katana_cairo::lang::starknet_classes::casm_contract_class::{
    CasmContractClass, StarknetSierraCompilationError,
};
use katana_cairo::lang::starknet_classes::contract_class::ContractEntryPoint;
use serde_json_pythonic::to_string_pythonic;
use starknet::core::utils::{normalize_address, starknet_keccak};
use starknet::macros::short_string;
use starknet_crypto::poseidon_hash_many;

use crate::Felt;

/// The canonical contract class (Sierra) type.
pub use ::katana_cairo::lang::starknet_classes::contract_class::ContractClass as SierraContractClass;
/// The canonical legacy class (Cairo 0) type.
pub use ::katana_cairo::starknet_api::deprecated_contract_class::ContractClass as LegacyContractClass;

/// The canonical hash of a contract class. This is the identifier of a class.
pub type ClassHash = Felt;
/// The hash of a compiled contract class.
pub type CompiledClassHash = Felt;

#[derive(Debug, thiserror::Error)]
pub enum ContractClassCompilationError {
    #[error(transparent)]
    SierraCompilation(#[from] StarknetSierraCompilationError),
}

#[derive(Debug, Clone, Eq, PartialEq, derive_more::From)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum ContractClass {
    Class(SierraContractClass),
    Legacy(LegacyContractClass),
}

impl ContractClass {
    pub fn class_hash(&self) -> Result<ClassHash, ComputeClassHashError> {
        match self {
            Self::Class(class) => compute_sierra_class_hash(class),
            Self::Legacy(class) => compute_legacy_class_hash(&class),
        }
    }

    /// Compiles the contract class into CASM.
    pub fn compile(self) -> Result<CompiledClass, ContractClassCompilationError> {
        match self {
            Self::Legacy(class) => Ok(CompiledClass::Legacy(class)),
            Self::Class(class) => {
                // let class = rpc_to_cairo_contract_class(&class).unwrap();
                let casm = CasmContractClass::from_contract_class(class, true, usize::MAX)?;
                Ok(CompiledClass::Class(casm))
            }
        }
    }

    /// Returns the class as a Sierra class, if any.
    pub fn as_class(&self) -> Option<&SierraContractClass> {
        match self {
            Self::Class(class) => Some(class),
            _ => None,
        }
    }

    /// Returns the class as a legacy class, if any.
    pub fn as_legacy(&self) -> Option<&LegacyContractClass> {
        match self {
            Self::Legacy(class) => Some(class),
            _ => None,
        }
    }
}

/// Compiled version of [`ContractClass`]. This is the format that is used for execution.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Eq, PartialEq, derive_more::From)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum CompiledClass {
    Class(CasmContractClass),
    Legacy(LegacyContractClass),
}

#[derive(Debug, thiserror::Error)]
pub enum ComputeClassHashError {
    #[error(transparent)]
    AbiConversion(#[from] serde_json_pythonic::Error),
}

// Taken from starknet-rs
fn compute_sierra_class_hash(class: &SierraContractClass) -> Result<Felt, ComputeClassHashError> {
    // Technically we don't have to use the Pythonic JSON style here. Doing this just to align
    // with the official `cairo-lang` CLI.
    //
    // TODO: add an `AbiFormatter` trait and let users choose which one to use.
    let abi = class.abi.as_ref();
    let abi_str = to_string_pythonic(abi.unwrap_or(&abi::Contract::default())).unwrap();

    let mut hasher = starknet_crypto::PoseidonHasher::new();
    hasher.update(short_string!("CONTRACT_CLASS_V0.1.0"));

    // Hashes entry points
    hasher.update(entrypoints_hash(&class.entry_points_by_type.external));
    hasher.update(entrypoints_hash(&class.entry_points_by_type.l1_handler));
    hasher.update(entrypoints_hash(&class.entry_points_by_type.constructor));

    // Hashes ABI
    hasher.update(starknet_keccak(abi_str.as_bytes()));

    // Hashes Sierra program
    let program =
        class.sierra_program.iter().map(|f| f.value.clone().into()).collect::<Vec<Felt>>();
    hasher.update(poseidon_hash_many(&program));

    Ok(normalize_address(hasher.finalize()))
}

fn entrypoints_hash(entrypoints: &[ContractEntryPoint]) -> Felt {
    let mut hasher = starknet_crypto::PoseidonHasher::new();

    for entry in entrypoints {
        hasher.update(entry.selector.clone().into());
        hasher.update(entry.function_idx.into());
    }

    hasher.finalize()
}

fn compute_legacy_class_hash(class: &LegacyContractClass) -> Result<Felt, ComputeClassHashError> {
    pub use starknet::core::types::contract::legacy::LegacyContractClass as StarknetRsLegacyContractClass;

    let value = serde_json::to_value(class).unwrap();
    let class = serde_json::from_value::<StarknetRsLegacyContractClass>(value).unwrap();
    let hash = class.class_hash().unwrap();

    Ok(hash)
}
