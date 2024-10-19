use anyhow::Result;
use bitvec::vec::BitVec;
use katana_primitives::Felt;
use starknet_types_core::hash::StarkHash;

pub fn compute_merkle_root<H>(values: &[Felt]) -> Result<Felt>
where
    H: StarkHash + Send + Sync,
{
    use bonsai_trie::id::BasicId;
    use bonsai_trie::{databases, BonsaiStorage, BonsaiStorageConfig};

    // TODO: replace the identifier by an empty slice when bonsai supports it
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
