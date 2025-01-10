use katana_cairo::lang::starknet_classes::abi;
use katana_cairo::lang::starknet_classes::casm_contract_class::StarknetSierraCompilationError;
use katana_cairo::lang::starknet_classes::contract_class::ContractEntryPoint;
use serde_json_pythonic::to_string_pythonic;
use starknet::core::utils::{normalize_address, starknet_keccak};
use starknet::macros::short_string;
use starknet_crypto::poseidon_hash_many;

use crate::Felt;

/// The canonical hash of a contract class. This is the identifier of a class.
pub type ClassHash = Felt;
/// The hash of a compiled contract class.
pub type CompiledClassHash = Felt;

/// The canonical contract class (Sierra) type.
pub type SierraContractClass = katana_cairo::lang::starknet_classes::contract_class::ContractClass;
/// The canonical legacy class (Cairo 0) type.
pub type LegacyContractClass = katana_cairo::starknet_api::deprecated_contract_class::ContractClass;

/// The canonical compiled Sierra contract class type.
pub type CasmContractClass =
    katana_cairo::lang::starknet_classes::casm_contract_class::CasmContractClass;

#[derive(Debug, thiserror::Error)]
pub enum ContractClassCompilationError {
    #[error(transparent)]
    SierraCompilation(#[from] StarknetSierraCompilationError),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum ContractClass {
    Class(SierraContractClass),
    Legacy(LegacyContractClass),
}

impl ContractClass {
    /// Computes the hash of the class.
    pub fn class_hash(&self) -> Result<ClassHash, ComputeClassHashError> {
        match self {
            Self::Class(class) => compute_sierra_class_hash(class),
            Self::Legacy(class) => compute_legacy_class_hash(class),
        }
    }

    /// Compiles the contract class.
    pub fn compile(self) -> Result<CompiledClass, ContractClassCompilationError> {
        match self {
            Self::Legacy(class) => Ok(CompiledClass::Legacy(class)),
            Self::Class(class) => {
                let casm = CasmContractClass::from_contract_class(class, true, usize::MAX)?;
                let casm = CompiledClass::Class(casm);
                Ok(casm)
            }
        }
    }

    /// Checks if this contract class is a Cairo 0 legacy class.
    ///
    /// Returns `true` if the contract class is a legacy class, `false` otherwise.
    pub fn is_legacy(&self) -> bool {
        matches!(self, Self::Legacy(_))
    }
}

/// Compiled version of [`ContractClass`].
///
/// This is the CASM format that can be used for execution. TO learn more about CASM, check out the
/// [Starknet docs].
///
/// [Starknet docs]: https://docs.starknet.io/architecture-and-concepts/smart-contracts/cairo-and-sierra/#why_do_we_need_casm
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Eq, PartialEq, derive_more::From)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum CompiledClass {
    /// The compiled Sierra contract class ie CASM.
    Class(CasmContractClass),

    /// The compiled legacy contract class.
    ///
    /// This is the same as the uncompiled legacy class because prior to Sierra,
    /// the classes were already in CASM format.
    Legacy(LegacyContractClass),
}

impl CompiledClass {
    /// Computes the hash of the compiled class.
    pub fn class_hash(&self) -> Result<CompiledClassHash, ComputeClassHashError> {
        match self {
            Self::Class(class) => Ok(class.compiled_class_hash()),
            Self::Legacy(class) => Ok(compute_legacy_class_hash(class)?),
        }
    }
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
    let prog = class.sierra_program.iter().map(|f| f.value.clone().into()).collect::<Vec<Felt>>();
    hasher.update(poseidon_hash_many(&prog));

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

/// Computes the hash of a legacy contract class.
///
/// This function delegates the computation to the `starknet-rs` library. Don't really care about
/// performance here because it's only for legacy classes, but we should definitely find to improve
/// this without introducing too much complexity.
fn compute_legacy_class_hash(class: &LegacyContractClass) -> Result<Felt, ComputeClassHashError> {
    pub use starknet::core::types::contract::legacy::LegacyContractClass as StarknetRsLegacyContractClass;

    let value = serde_json::to_value(class).unwrap();
    let class = serde_json::from_value::<StarknetRsLegacyContractClass>(value).unwrap();
    let hash = class.class_hash().unwrap();

    Ok(hash)
}

#[cfg(test)]
mod tests {

    use starknet::core::types::contract::legacy::LegacyContractClass as StarknetRsLegacyContractClass;
    use starknet::core::types::contract::SierraClass as StarknetRsSierraContractClass;

    use super::{ContractClass, LegacyContractClass, SierraContractClass};

    #[test]
    fn compute_class_hash() {
        let artifact = include_str!("../../contracts/build/default_account.json");

        let class = serde_json::from_str::<SierraContractClass>(artifact).unwrap();
        let actual_hash = ContractClass::Class(class).class_hash().unwrap();

        // Compare it against the hash computed using `starknet-rs` types

        let class = serde_json::from_str::<StarknetRsSierraContractClass>(artifact).unwrap();
        let expected_hash = class.class_hash().unwrap();

        assert_eq!(actual_hash, expected_hash);
    }

    #[test]
    fn compute_legacy_class_hash() {
        let artifact = include_str!("../../contracts/build/erc20.json");

        let class = serde_json::from_str::<LegacyContractClass>(artifact).unwrap();
        let actual_hash = ContractClass::Legacy(class).class_hash().unwrap();

        // Compare it against the hash computed using `starknet-rs` types

        let class = serde_json::from_str::<StarknetRsLegacyContractClass>(artifact).unwrap();
        let expected_hash = class.class_hash().unwrap();

        assert_eq!(actual_hash, expected_hash);
    }
}
