use core::fmt;

use bitvec::array::BitArray;
use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::Felt;
use katana_trie::bonsai::id::BasicId;
use katana_trie::bonsai::{BonsaiStorage, BonsaiStorageConfig};
use katana_trie::MultiProof;
use starknet::macros::short_string;
use starknet_types_core::hash::{Poseidon, StarkHash};

use crate::abstraction::DbTxMut;
use crate::tables;
use crate::trie::TrieDb;

// https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-state/#classes_trie
const CONTRACT_CLASS_LEAF_V0: Felt = short_string!("CONTRACT_CLASS_LEAF_V0");

#[derive(Debug)]
pub struct ClassTrie<Tx>
where
    Tx: DbTxMut + fmt::Debug,
{
    inner: BonsaiStorage<BasicId, TrieDb<tables::ClassTrie, Tx>, Poseidon>,
}

impl<Tx> ClassTrie<Tx>
where
    Tx: DbTxMut + fmt::Debug,
{
    pub fn new(tx: Tx) -> Self {
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };

        let db = TrieDb::<tables::ClassTrie, Tx>::new(tx);
        let inner = BonsaiStorage::new(db, config, 64);

        Self { inner }
    }

    pub fn new_at_block(tx: Tx, block_number: BlockNumber) -> Option<Self> {
        let trie = Self::new(tx);
        trie.inner
            .get_transactional_state(BasicId::new(block_number), trie.inner.get_config())
            .expect("failed to get trie at exact block")
            .map(|inner| Self { inner })
    }

    pub fn insert(&mut self, hash: ClassHash, compiled_hash: CompiledClassHash) {
        let value = Poseidon::hash(&CONTRACT_CLASS_LEAF_V0, &compiled_hash);
        let key: BitVec<u8, Msb0> = hash.to_bytes_be().as_bits()[5..].to_owned();
        self.inner.insert(self.bonsai_identifier(), &key, &value).unwrap();
    }

    pub fn commit(&mut self, block_number: BlockNumber) {
        self.inner.commit(BasicId::new(block_number)).unwrap();
    }

    pub fn root(&self) -> Felt {
        self.inner.root_hash(self.bonsai_identifier()).unwrap()
    }

    pub fn get_multi_proof(&mut self, class_hashes: &[ClassHash]) -> MultiProof {
        let mut keys: Vec<BitArray<_, Msb0>> =
            class_hashes.iter().map(|h| BitArray::new(h.to_bytes_be())).collect();
        keys.sort();

        let keys = keys.iter().map(|hash| hash.as_bitslice()[5..].to_owned());
        let proofs = self.inner.get_multi_proof(&self.bonsai_identifier(), keys).unwrap();
        proofs
    }

    fn bonsai_identifier(&self) -> &'static [u8] {
        b"1"
    }
}
