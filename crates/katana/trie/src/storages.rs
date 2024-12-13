use bonsai_trie::{BonsaiDatabase, BonsaiPersistentDatabase, MultiProof};
use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{StorageKey, StorageValue};
use katana_primitives::{ContractAddress, Felt};

use crate::id::CommitId;

pub struct StoragesTrie<DB: BonsaiDatabase> {
    pub trie: crate::BonsaiTrie<DB>,
}

impl<DB: BonsaiDatabase> StoragesTrie<DB> {
    pub fn root(&self, address: ContractAddress) -> Felt {
        self.trie.root(&address.to_bytes_be())
    }

    pub fn multiproof(
        &mut self,
        address: ContractAddress,
        storage_keys: Vec<StorageKey>,
    ) -> MultiProof {
        self.trie.multiproof(&address.to_bytes_be(), storage_keys)
    }
}

impl<DB> StoragesTrie<DB>
where
    DB: BonsaiDatabase + BonsaiPersistentDatabase<CommitId>,
{
    pub fn insert(
        &mut self,
        address: ContractAddress,
        storage_key: StorageKey,
        storage_value: StorageValue,
    ) {
        self.trie.insert(&address.to_bytes_be(), storage_key, storage_value)
    }

    pub fn commit(&mut self, block: BlockNumber) {
        self.trie.commit(block.into())
    }
}
