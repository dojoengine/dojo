use bonsai_trie::{BonsaiDatabase, BonsaiPersistentDatabase, MultiProof};
use katana_primitives::block::BlockNumber;
use katana_primitives::hash::Pedersen;
use katana_primitives::{ContractAddress, Felt};

use crate::id::CommitId;

#[derive(Debug)]
pub struct ContractsTrie<DB: BonsaiDatabase> {
    trie: crate::BonsaiTrie<DB, Pedersen>,
}

//////////////////////////////////////////////////////////////
// 	ContractsTrie implementations
//////////////////////////////////////////////////////////////

impl<DB: BonsaiDatabase> ContractsTrie<DB> {
    /// NOTE: The identifier value is only relevant if the underlying [`BonsaiDatabase`]
    /// implementation is shared across other tries.
    const BONSAI_IDENTIFIER: &'static [u8] = b"contracts";

    pub fn new(db: DB) -> Self {
        Self { trie: crate::BonsaiTrie::new(db) }
    }

    pub fn root(&self) -> Felt {
        self.trie.root(Self::BONSAI_IDENTIFIER)
    }

    pub fn multiproof(&mut self, addresses: Vec<ContractAddress>) -> MultiProof {
        let keys = addresses.into_iter().map(Felt::from).collect::<Vec<Felt>>();
        self.trie.multiproof(Self::BONSAI_IDENTIFIER, keys)
    }
}

impl<DB> ContractsTrie<DB>
where
    DB: BonsaiDatabase + BonsaiPersistentDatabase<CommitId>,
{
    pub fn insert(&mut self, address: ContractAddress, state_hash: Felt) {
        self.trie.insert(Self::BONSAI_IDENTIFIER, *address, state_hash)
    }

    pub fn commit(&mut self, block: BlockNumber) {
        self.trie.commit(block.into())
    }
}

#[derive(Debug, Default)]
pub struct ContractLeaf {
    pub class_hash: Option<Felt>,
    pub storage_root: Option<Felt>,
    pub nonce: Option<Felt>,
}
