pub use katana_cairo::lang::starknet_classes::casm_contract_class::CasmContractClass;
use katana_cairo::lang::starknet_classes::casm_contract_class::StarknetSierraCompilationError;

use crate::conversion::rpc::rpc_to_cairo_contract_class;

/// The canonical hash of a contract class. This is the identifier of a class.
pub type ClassHash = crate::Felt;
/// The hash of a compiled contract class.
pub type CompiledClassHash = crate::Felt;

pub type SierraClass = starknet::core::types::contract::SierraClass;
pub type FlattenedSierraClass = starknet::core::types::FlattenedSierraClass;

/// Deprecated legacy (Cairo 0) class
pub type LegacyContractClass =
    ::katana_cairo::starknet_api::deprecated_contract_class::ContractClass;

#[derive(Debug, thiserror::Error)]
pub enum ContractClassCompilationError {
    #[error(transparent)]
    SierraCompilation(#[from] StarknetSierraCompilationError),
}

#[derive(Debug, Clone, Eq, PartialEq, derive_more::From)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub enum ContractClass {
    Class(FlattenedSierraClass),
    Legacy(LegacyContractClass),
}

impl ContractClass {
    /// Compiles the contract class into CASM.
    pub fn compile(self) -> Result<CompiledClass, ContractClassCompilationError> {
        match self {
            Self::Legacy(class) => Ok(CompiledClass::Legacy(class)),
            Self::Class(class) => {
                let class = rpc_to_cairo_contract_class(&class).unwrap();
                let casm = CasmContractClass::from_contract_class(class, true, usize::MAX)?;
                Ok(CompiledClass::Class(casm))
            }
        }
    }

    /// Returns the class as a Sierra class, if any.
    pub fn as_class(&self) -> Option<&FlattenedSierraClass> {
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
