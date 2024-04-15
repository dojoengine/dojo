use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{ContractAddress, StorageKey};

use super::{DupSort, Table};
use crate::codecs::{Compress, Decode, Decompress, Encode};
use crate::error::CodecError;
use crate::models::contract::{ContractClassChange, ContractNonceChange};
use crate::models::storage::ContractStorageEntry;
use crate::models::storage::ContractStorageKey;
use crate::{dupsort, tables};

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

#[derive(Debug)]
pub struct StorageEntryChangeList {
    pub key: StorageKey,
    pub block_list: Vec<BlockNumber>,
}

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
