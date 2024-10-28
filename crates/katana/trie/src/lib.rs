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
