use katana_primitives::block::BlockNumber;
use katana_primitives::contract::ContractAddress;

use super::{DupSort, Table};
use crate::models::contract::{ContractClassChange, ContractNonceChange};
use crate::models::storage::ContractStorageEntry;
use crate::models::storage::ContractStorageKey;
use crate::{dupsort, tables};

tables! {
    /// Contract nonce changes by block.
    NonceChanges: (BlockNumber, ContractAddress) => ContractNonceChange,
    /// Contract class hash changes by block.
    ContractClassChanges: (BlockNumber, ContractAddress) => ContractClassChange,
    /// Account storage change set
    StorageChanges: (BlockNumber, ContractStorageKey) => ContractStorageEntry
}
