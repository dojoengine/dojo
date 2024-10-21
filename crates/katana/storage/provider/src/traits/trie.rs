use std::collections::BTreeMap;

use katana_primitives::class::{ClassHash, CompiledClassHash};

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ClassTrieWriter: Send + Sync {
    fn insert_updates(
        &self,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> ProviderResult<()>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractTrieWriter: Send + Sync {
    fn insert_updates(&self) -> ProviderResult<()>;
}
