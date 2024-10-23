use std::collections::BTreeMap;

use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ClassTrieWriter: Send + Sync {
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> ProviderResult<()>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractTrieWriter: Send + Sync {
    fn insert_updates(&self) -> ProviderResult<()>;
}
