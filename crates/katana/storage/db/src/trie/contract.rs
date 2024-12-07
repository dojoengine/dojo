use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;
use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{StorageKey, StorageValue};
use katana_primitives::{ContractAddress, Felt};
use katana_trie::bonsai::id::BasicId;
use katana_trie::bonsai::{BonsaiStorage, BonsaiStorageConfig};
use starknet_types_core::hash::Poseidon;

use crate::abstraction::DbTxMut;
use crate::tables;
use crate::trie::TrieDb;

#[derive(Debug)]
pub struct StorageTrie<Tx: DbTxMut> {
    inner: BonsaiStorage<BasicId, TrieDb<tables::ContractStorageTrie, Tx>, Poseidon>,
}

impl<Tx: DbTxMut> StorageTrie<Tx> {
    pub fn new(tx: Tx) -> Self {
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };

        let db = TrieDb::<tables::ContractStorageTrie, Tx>::new(tx);
        let inner = BonsaiStorage::new(db, config).unwrap();

        Self { inner }
    }

    pub fn insert(&mut self, address: ContractAddress, key: StorageKey, value: StorageValue) {
        let key: BitVec<u8, Msb0> = key.to_bytes_be().as_bits()[5..].to_owned();
        self.inner.insert(&address.to_bytes_be(), &key, &value).unwrap();
    }

    pub fn commit(&mut self, block_number: BlockNumber) {
        self.inner.commit(BasicId::new(block_number)).unwrap();
    }

    pub fn root(&self, address: &ContractAddress) -> Felt {
        self.inner.root_hash(&address.to_bytes_be()).unwrap()
    }
}

#[derive(Debug)]
pub struct ContractTrie<Tx: DbTxMut> {
    inner: BonsaiStorage<BasicId, TrieDb<tables::ContractTrie, Tx>, Poseidon>,
}

impl<Tx: DbTxMut> ContractTrie<Tx> {
    pub fn new(tx: Tx) -> Self {
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };

        let db = TrieDb::<tables::ContractTrie, Tx>::new(tx);
        let inner = BonsaiStorage::new(db, config).unwrap();

        Self { inner }
    }

    pub fn insert(&mut self, address: ContractAddress, state_hash: Felt) {
        let key: BitVec<u8, Msb0> = address.to_bytes_be().as_bits()[5..].to_owned();
        self.inner.insert(self.bonsai_identifier(), &key, &state_hash).unwrap();
    }

    pub fn commit(&mut self, block_number: BlockNumber) {
        self.inner.commit(BasicId::new(block_number)).unwrap();
    }

    pub fn root(&self) -> Felt {
        self.inner.root_hash(self.bonsai_identifier()).unwrap()
    }

    fn bonsai_identifier(&self) -> &'static [u8] {
        b"1"
    }
}
