use std::collections::BTreeMap;

use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;
use bonsai_trie::id::BasicId;
use bonsai_trie::{
    BonsaiDatabase, BonsaiPersistentDatabase, BonsaiStorage, BonsaiStorageConfig, ByteVec,
    DatabaseKey,
};
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::Felt;
use starknet::macros::short_string;
use starknet_types_core::hash::{Poseidon, StarkHash};

// https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-state/#classes_trie
const CONTRACT_CLASS_LEAF_V0: Felt = short_string!("CONTRACT_CLASS_LEAF_V0");

pub struct ClassTrie<BD>
where
    BD: BonsaiDatabase + BonsaiPersistentDatabase<BasicId>,
{
    bonsai_storage: BonsaiStorage<BasicId, BD, Poseidon>,
}

impl<BD> ClassTrie<BD>
where
    BD: BonsaiDatabase + BonsaiPersistentDatabase<BasicId>,
{
    const IDENTIFIER: &'static [u8] = b"0xclass";

    pub fn new(bd: BD) -> Self {
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };
        Self { bonsai_storage: BonsaiStorage::new(bd, config).unwrap() }
    }

    pub fn apply(
        &mut self,
        block_number: BlockNumber,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> Felt {
        let updates: Vec<_> = updates
            .into_iter()
            .map(|(class_hash, compiled_class_hash)| {
                let hash = Poseidon::hash(&CONTRACT_CLASS_LEAF_V0, compiled_class_hash);
                (*class_hash, hash)
            })
            .collect();

        for (key, value) in updates {
            let bytes = key.to_bytes_be();
            let bv: BitVec<u8, Msb0> = bytes.as_bits()[5..].to_owned();
            self.bonsai_storage.insert(Self::IDENTIFIER, &bv, &value).unwrap();
        }

        self.bonsai_storage.commit(BasicId::new(block_number)).unwrap();
        let root_hash = self.bonsai_storage.root_hash(Self::IDENTIFIER).unwrap();
        root_hash
    }
}
