use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{ContractAddress, StorageKey};

use super::*;
use crate::codecs::{Compress, Decode, Decompress, Encode};
use crate::error::CodecError;
use crate::models::contract::{ContractClassChange, ContractNonceChange};
use crate::models::storage::ContractStorageEntry;
use crate::models::storage::ContractStorageKey;
use crate::{define_tables_enum, dupsort, tables};

// TODO(kariy): can we somehow define this without repeating existing tables, and only add the older ones?
define_tables_enum! {[
    (Headers, TableType::Table),
    (BlockHashes, TableType::Table),
    (BlockNumbers, TableType::Table),
    (BlockBodyIndices, TableType::Table),
    (BlockStatusses, TableType::Table),
    (TxNumbers, TableType::Table),
    (TxBlocks, TableType::Table),
    (TxHashes, TableType::Table),
    (Transactions, TableType::Table),
    (Receipts, TableType::Table),
    (CompiledClassHashes, TableType::Table),
    (CompiledClasses, TableType::Table),
    (SierraClasses, TableType::Table),
    (ContractInfo, TableType::Table),
    (ContractStorage, TableType::DupSort),
    (ClassDeclarationBlock, TableType::Table),
    (ClassDeclarations, TableType::DupSort),
    (ContractInfoChangeSet, TableType::Table),
    (NonceChanges, TableType::DupSort),
    (ContractClassChanges, TableType::DupSort),
    (StorageChanges, TableType::DupSort),
    (StorageChangeSet, TableType::DupSort)
]}

tables! {
    /// Contract nonce changes by block.
    NonceChanges: (BlockNumber, ContractAddress) => ContractNonceChange,
    /// Contract class hash changes by block.
    ContractClassChanges: (BlockNumber, ContractAddress) => ContractClassChange,

    /// storage change set
    StorageChangeSet: (ContractAddress, StorageKey) => StorageEntryChangeList,
    /// Account storage change set
    StorageChanges: (BlockNumber, ContractStorageKey) => ContractStorageEntry
}

pub type BlockList = Vec<BlockNumber>;

/// This is used as a value type for the [`StorageChangeSet`] dupsort table
#[derive(Debug)]
pub struct StorageEntryChangeList {
    pub key: StorageKey,
    pub block_list: Vec<BlockNumber>,
}

// The `key` field is the subkey of the dupsort table, so we must use
// the Encode and Decode traits  when de/serializing it to the database.
impl Compress for StorageEntryChangeList {
    type Compressed = Vec<u8>;
    fn compress(self) -> Self::Compressed {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.key.encode());
        buf.extend_from_slice(&self.block_list.compress());
        buf
    }
}

impl Decompress for StorageEntryChangeList {
    fn decompress<B: AsRef<[u8]>>(bytes: B) -> Result<Self, CodecError> {
        let bytes = bytes.as_ref();
        let key = StorageKey::decode(&bytes[0..32])?;
        let blocks = Vec::<BlockNumber>::decompress(&bytes[32..])?;
        Ok(Self { key, block_list: blocks })
    }
}
