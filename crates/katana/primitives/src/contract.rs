use std::fmt;

use derive_more::Deref;
use starknet::core::utils::normalize_address;

use crate::class::ClassHash;
use crate::FieldElement;

/// Represents the type for a contract storage key.
pub type StorageKey = FieldElement;
/// Represents the type for a contract storage value.
pub type StorageValue = FieldElement;

/// Represents the type for a contract nonce.
pub type Nonce = FieldElement;

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
