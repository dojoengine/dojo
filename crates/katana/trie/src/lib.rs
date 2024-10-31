use anyhow::Result;
use bitvec::vec::BitVec;
pub use bonsai_trie as bonsai;
use bonsai_trie::id::BasicId;
use bonsai_trie::{BonsaiDatabase, BonsaiPersistentDatabase};
use katana_primitives::Felt;
use starknet_types_core::hash::StarkHash;

mod class;
mod contract;

pub use class::ClassTrie;

/// A helper trait to define a database that can be used as a Bonsai Trie.
///
/// Basically a short hand for `BonsaiDatabase + BonsaiPersistentDatabase<BasicId>`.
pub trait BonsaiTrieDb: BonsaiDatabase + BonsaiPersistentDatabase<BasicId> {}
impl<T> BonsaiTrieDb for T where T: BonsaiDatabase + BonsaiPersistentDatabase<BasicId> {}

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
    let mut bs = BonsaiStorage::<_, _, H>::new(bonsai_db, config).unwrap();

    for (id, value) in values.iter().enumerate() {
        let key = BitVec::from_iter(id.to_be_bytes());
        bs.insert(IDENTIFIER, key.as_bitslice(), value).unwrap();
    }

    let id = bonsai_trie::id::BasicIdBuilder::new().new_id();
    bs.commit(id).unwrap();

    Ok(bs.root_hash(IDENTIFIER).unwrap())
}

#[cfg(test)]
mod tests {

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
}
