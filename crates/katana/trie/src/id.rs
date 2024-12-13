use bonsai_trie::id::Id;
use bonsai_trie::ByteVec;
use katana_primitives::block::BlockNumber;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct CommitId(BlockNumber);

impl CommitId {
    pub fn new(block_number: BlockNumber) -> Self {
        Self(block_number)
    }
}

impl Id for CommitId {
    fn as_u64(self) -> u64 {
        self.0
    }

    fn from_u64(v: u64) -> Self {
        Self(v)
    }

    fn to_bytes(&self) -> ByteVec {
        ByteVec::from(&self.0.to_be_bytes() as &[_])
    }
}

impl From<BlockNumber> for CommitId {
    fn from(value: BlockNumber) -> Self {
        Self(value)
    }
}
