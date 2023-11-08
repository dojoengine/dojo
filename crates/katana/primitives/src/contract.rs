use std::fmt;

use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use starknet::core::utils::normalize_address;

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

pub type SierraClass = starknet::core::types::FlattenedSierraClass;

/// Represents a contract address.
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ContractAddress(FieldElement);

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericContractInfo {
    /// The nonce of the contract.
    pub nonce: Nonce,
    /// The hash of the contract class.
    pub class_hash: ClassHash,
}
