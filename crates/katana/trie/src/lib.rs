use anyhow::Result;
use bitvec::array::BitArray;
use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;
pub use bonsai::{MultiProof, ProofNode};
pub use bonsai_trie as bonsai;
pub use bonsai_trie::{BonsaiDatabase, BonsaiPersistentDatabase};
use bonsai_trie::{BonsaiStorage, BonsaiStorageConfig};
use katana_primitives::class::ClassHash;
use katana_primitives::Felt;
use starknet_types_core::hash::{Pedersen, Poseidon, StarkHash};

mod classes;
mod contracts;
mod id;
mod storages;

pub use classes::ClassesTrie;
pub use contracts::ContractsTrie;
pub use id::CommitId;
pub use storages::StoragesTrie;

/// A lightweight shim for [`BonsaiStorage`]
pub struct BonsaiTrie<DB>
where
    DB: BonsaiDatabase,
{
    storage: BonsaiStorage<CommitId, DB, Poseidon>,
}

impl<DB> BonsaiTrie<DB>
where
    DB: BonsaiDatabase,
{
    pub fn new(db: DB) -> Self {
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };

        Self { storage: BonsaiStorage::new(db, config, 251) }
    }
}

impl<DB> BonsaiTrie<DB>
where
    DB: BonsaiDatabase,
{
    pub fn root(&self, id: &[u8]) -> Felt {
        self.storage.root_hash(id).expect("failed to get trie root")
    }

    pub fn multiproof(&mut self, id: &[u8], mut keys: Vec<Felt>) -> MultiProof {
        keys.sort();
        let keys = keys
            .into_iter()
            .map(|key| BitArray::new(key.to_bytes_be()))
            .map(|hash| hash.as_bitslice()[5..].to_owned());

        self.storage.get_multi_proof(id, keys).expect("failed to get multiproof")
    }
}

impl<DB> BonsaiTrie<DB>
where
    DB: BonsaiDatabase + BonsaiPersistentDatabase<CommitId>,
{
    pub fn insert(&mut self, id: &[u8], key: Felt, value: Felt) {
        let key: BitVec<u8, Msb0> = key.to_bytes_be().as_bits()[5..].to_owned();
        self.storage.insert(id, &key, &value).unwrap();
    }

    pub fn commit(&mut self, id: CommitId) {
        self.storage.commit(id).expect("failed to commit trie");
    }
}

pub fn compute_merkle_root<H>(values: &[Felt]) -> Result<Felt>
where
    H: StarkHash + Send + Sync,
{
    use bonsai_trie::id::BasicId;
    use bonsai_trie::{databases, BonsaiStorage, BonsaiStorageConfig};

    // the value is irrelevant
    const IDENTIFIER: &[u8] = b"1";

    let config = BonsaiStorageConfig::default();
    let bonsai_db = databases::HashMapDb::<BasicId>::default();
    let mut bs = BonsaiStorage::<_, _, H>::new(bonsai_db, config, 251);

    for (id, value) in values.iter().enumerate() {
        let key = BitVec::from_iter(id.to_be_bytes());
        bs.insert(IDENTIFIER, key.as_bitslice(), value).unwrap();
    }

    let id = bonsai_trie::id::BasicIdBuilder::new().new_id();
    bs.commit(id).unwrap();

    Ok(bs.root_hash(IDENTIFIER).unwrap())
}

// H(H(H(class_hash, storage_root), nonce), 0), where H is the pedersen hash
pub fn compute_contract_state_hash(
    class_hash: &ClassHash,
    storage_root: &Felt,
    nonce: &Felt,
) -> Felt {
    const CONTRACT_STATE_HASH_VERSION: Felt = Felt::ZERO;
    let hash = Pedersen::hash(class_hash, storage_root);
    let hash = Pedersen::hash(&hash, nonce);
    Pedersen::hash(&hash, &CONTRACT_STATE_HASH_VERSION)
}

#[cfg(test)]
mod tests {

    use katana_primitives::contract::Nonce;
    use katana_primitives::felt;
    use starknet_types_core::hash;

    use super::*;

    // Taken from Pathfinder: https://github.com/eqlabs/pathfinder/blob/29f93d0d6ad8758fdcf5ae3a8bd2faad2a3bc92b/crates/merkle-tree/src/transaction.rs#L70-L88
    #[test]
    fn test_commitment_merkle_tree() {
        let hashes = vec![Felt::from(1), Felt::from(2), Felt::from(3), Felt::from(4)];

        // Produced by the cairo-lang Python implementation:
        // `hex(asyncio.run(calculate_patricia_root([1, 2, 3, 4], height=64, ffc=ffc))))`
        let expected_root_hash =
            felt!("0x1a0e579b6b444769e4626331230b5ae39bd880f47e703b73fa56bf77e52e461");
        let computed_root_hash = compute_merkle_root::<hash::Pedersen>(&hashes).unwrap();

        assert_eq!(expected_root_hash, computed_root_hash);
    }

    // Taken from Pathfinder: https://github.com/eqlabs/pathfinder/blob/29f93d0d6ad8758fdcf5ae3a8bd2faad2a3bc92b/crates/merkle-tree/src/contract_state.rs#L236C5-L252C6
    #[test]
    fn test_compute_contract_state_hash() {
        let root = felt!("0x4fb440e8ca9b74fc12a22ebffe0bc0658206337897226117b985434c239c028");
        let class_hash = felt!("0x2ff4903e17f87b298ded00c44bfeb22874c5f73be2ced8f1d9d9556fb509779");
        let nonce = Nonce::ZERO;

        let result = compute_contract_state_hash(&class_hash, &root, &nonce);
        let expected = felt!("0x7161b591c893836263a64f2a7e0d829c92f6956148a60ce5e99a3f55c7973f3");

        assert_eq!(result, expected);
    }
}
