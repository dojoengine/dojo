use core::fmt;

use bitvec::array::BitArray;
use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;
use katana_primitives::block::BlockNumber;
use katana_primitives::contract::{StorageKey, StorageValue};
use katana_primitives::{ContractAddress, Felt};
use katana_trie::bonsai::id::BasicId;
use katana_trie::bonsai::{BonsaiStorage, BonsaiStorageConfig};
use katana_trie::MultiProof;
use starknet_types_core::hash::Poseidon;

use crate::abstraction::DbTxMut;
use crate::tables;
use crate::trie::TrieDb;

#[derive(Debug)]
pub struct StorageTrie<Tx>
where
    Tx: DbTxMut + fmt::Debug,
{
    inner: BonsaiStorage<BasicId, TrieDb<tables::ContractStorageTrie, Tx>, Poseidon>,
}

impl<Tx> StorageTrie<Tx>
where
    Tx: DbTxMut + fmt::Debug,
{
    pub fn new(tx: Tx) -> Self {
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };

        let db = TrieDb::<tables::ContractStorageTrie, Tx>::new(tx);
        let inner = BonsaiStorage::new(db, config, 251);

        Self { inner }
    }

    pub fn new_at_block(tx: Tx, block_number: BlockNumber) -> Option<Self> {
        let trie = Self::new(tx);
        trie.inner
            .get_transactional_state(BasicId::new(block_number), trie.inner.get_config())
            .expect("failed to get trie at exact block")
            .map(|inner| Self { inner })
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

    pub fn get_multi_proof(
        &mut self,
        address: ContractAddress,
        key: Vec<StorageKey>,
    ) -> MultiProof {
        let mut keys: Vec<BitArray<_, Msb0>> =
            key.iter().map(|k| BitArray::new(k.to_bytes_be())).collect();
        keys.sort();

        let keys = keys.iter().map(|k| k.as_bitslice()[5..].to_owned());
        self.inner.get_multi_proof(&address.to_bytes_be(), keys).unwrap()
    }
}

#[derive(Debug)]
pub struct ContractTrie<Tx>
where
    Tx: DbTxMut + fmt::Debug,
{
    inner: BonsaiStorage<BasicId, TrieDb<tables::ContractTrie, Tx>, Poseidon>,
}

impl<Tx> ContractTrie<Tx>
where
    Tx: DbTxMut + fmt::Debug,
{
    pub fn new(tx: Tx) -> Self {
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };

        let db = TrieDb::<tables::ContractTrie, Tx>::new(tx);
        let inner = BonsaiStorage::new(db, config, 251);

        Self { inner }
    }

    pub fn new_at_block(tx: Tx, block_number: BlockNumber) -> Option<Self> {
        let trie = Self::new(tx);
        trie.inner
            .get_transactional_state(BasicId::new(block_number), trie.inner.get_config())
            .expect("failed to get trie at exact block")
            .map(|inner| Self { inner })
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

    pub fn get_multi_proof(&mut self, contract_addresses: &[ContractAddress]) -> MultiProof {
        let mut keys: Vec<BitArray<_, Msb0>> =
            contract_addresses.iter().map(|h| BitArray::new(h.to_bytes_be())).collect();
        keys.sort();

        let keys = keys.iter().map(|hash| hash.as_bitslice()[5..].to_owned());
        let proofs = self.inner.get_multi_proof(&self.bonsai_identifier(), keys).unwrap();
        proofs
    }

    fn bonsai_identifier(&self) -> &'static [u8] {
        b"1"
    }
}
