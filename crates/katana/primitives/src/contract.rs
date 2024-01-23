use std::fmt;

use derive_more::Deref;
use lazy_static::lazy_static;
use starknet::core::utils::normalize_address;
use starknet::macros::felt;

use crate::utils::class::{parse_compiled_class, parse_sierra_class};
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

/// Represents a runnable Starknet contract class (meaning, the program is runnable by the VM).
#[cfg(feature = "blockifier")]
pub type CompiledContractClass = ::blockifier::execution::contract_class::ContractClass;
/// V0 of the compiled contract class
#[cfg(feature = "blockifier")]
pub type CompiledContractClassV0 = ::blockifier::execution::contract_class::ContractClassV0;
/// V1 of the compiled contract class
#[cfg(feature = "blockifier")]
pub type CompiledContractClassV1 = ::blockifier::execution::contract_class::ContractClassV1;

lazy_static! {

    // Pre-compiled contract classes

    pub static ref LEGACY_ERC20_CONTRACT_CASM: CompiledContractClass = parse_compiled_class(include_str!("../contracts/compiled/erc20.json")).unwrap();
    pub static ref LEGACY_ERC20_CONTRACT_CLASS_HASH: ClassHash = felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");
    pub static ref LEGACY_ERC20_CONTRACT_COMPILED_CLASS_HASH: CompiledClassHash = felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f");

    pub static ref LEGACY_UDC_CASM: CompiledContractClass = parse_compiled_class(include_str!("../contracts/compiled/universal_deployer.json")).unwrap();
    pub static ref LEGACY_UDC_CLASS_HASH: ClassHash = felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");
    pub static ref LEGACY_UDC_COMPILED_CLASS_HASH: CompiledClassHash = felt!("0x07b3e05f48f0c69e4a65ce5e076a66271a527aff2c34ce1083ec6e1526997a69");

    pub static ref OZ_ACCOUNT_CONTRACT: SierraClass = parse_sierra_class(include_str!("../contracts/compiled/oz_account_080.json")).unwrap();
    pub static ref OZ_ACCOUNT_CONTRACT_CASM: CompiledContractClass = parse_compiled_class(include_str!("../contracts/compiled/oz_account_080.json")).unwrap();
    pub static ref OZ_ACCOUNT_CONTRACT_CLASS_HASH: ClassHash = felt!("0x05400e90f7e0ae78bd02c77cd75527280470e2fe19c54970dd79dc37a9d3645c");
    pub static ref OZ_ACCOUNT_CONTRACT_COMPILED_CLASS_HASH: CompiledClassHash = felt!("0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc");

}
