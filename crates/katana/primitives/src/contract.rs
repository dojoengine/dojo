use std::fmt;

use derive_more::Deref;
use starknet::core::utils::normalize_address;

use crate::class::ClassHash;
use crate::Felt;

/// Represents the type for a contract storage key.
pub type StorageKey = Felt;
/// Represents the type for a contract storage value.
pub type StorageValue = Felt;

/// Represents the type for a contract nonce.
pub type Nonce = Felt;

/// Represents the type for a message hash.
pub type MessageHash = Felt;

/// Represents a contract address.
#[derive(Default, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Deref)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContractAddress(#[deref] pub Felt);

impl ContractAddress {
    pub fn new(address: Felt) -> Self {
        ContractAddress(normalize_address(address))
    }
}

impl fmt::Display for ContractAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl From<Felt> for ContractAddress {
    fn from(value: Felt) -> Self {
        ContractAddress::new(value)
    }
}

impl From<ContractAddress> for Felt {
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
