use bonsai_trie::{BonsaiDatabase, BonsaiPersistentDatabase, MultiProof};
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::hash::Pedersen;
use katana_primitives::Felt;
use starknet::macros::short_string;
use starknet_types_core::hash::{Poseidon, StarkHash};

use crate::id::CommitId;

#[derive(Debug)]
pub struct ClassesTrie<DB: BonsaiDatabase> {
    trie: crate::BonsaiTrie<DB, Pedersen>,
}

impl<DB: BonsaiDatabase> ClassesTrie<DB> {
    const BONSAI_IDENTIFIER: &'static [u8] = b"classes";

    pub fn new(db: DB) -> Self {
        Self { trie: crate::BonsaiTrie::new(db) }
    }

    pub fn root(&self) -> Felt {
        self.trie.root(Self::BONSAI_IDENTIFIER)
    }

    pub fn multiproof(&mut self, class_hashes: Vec<ClassHash>) -> MultiProof {
        self.trie.multiproof(Self::BONSAI_IDENTIFIER, class_hashes)
    }
}

impl<DB> ClassesTrie<DB>
where
    DB: BonsaiDatabase + BonsaiPersistentDatabase<CommitId>,
{
    pub fn insert(&mut self, hash: ClassHash, compiled_hash: CompiledClassHash) {
        // https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-state/#classes_trie
        const CONTRACT_CLASS_LEAF_V0: Felt = short_string!("CONTRACT_CLASS_LEAF_V0");
        let value = Poseidon::hash(&CONTRACT_CLASS_LEAF_V0, &compiled_hash);
        self.trie.insert(Self::BONSAI_IDENTIFIER, hash, value)
    }

    pub fn commit(&mut self, block: BlockNumber) {
        self.trie.commit(block.into())
    }
}
