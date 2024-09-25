use std::fmt;

use starknet::core::utils::normalize_address;

use crate::class::ClassHash;
use crate::Felt;

/// Represents the type for a contract storage key.
pub type StorageKey = Felt;
/// Represents the type for a contract storage value.
pub type StorageValue = Felt;

/// Represents the type for a contract nonce.
pub type Nonce = Felt;

/// Represents a contract address.
#[derive(Default, Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ContractAddress(pub Felt);

impl ContractAddress {
    pub fn new(address: Felt) -> Self {
        ContractAddress(normalize_address(address))
    }
}

impl core::ops::Deref for ContractAddress {
    type Target = Felt;

    fn deref(&self) -> &Self::Target {
        &self.0
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

#[macro_export]
macro_rules! address {
    ($value:expr) => {
        ContractAddress::new($crate::felt!($value))
    };
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
