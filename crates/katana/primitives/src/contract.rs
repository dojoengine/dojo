use std::fmt;

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

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Debug)]
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
