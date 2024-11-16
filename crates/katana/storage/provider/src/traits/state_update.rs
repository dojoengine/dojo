use std::collections::BTreeMap;

use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::state::StateUpdates;
use katana_primitives::ContractAddress;

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateUpdateProvider: Send + Sync {
    /// Returns the state update at the given block.
    fn state_update(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<StateUpdates>>;

    /// Returns all declared class hashes at the given block.
    fn declared_classes(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BTreeMap<ClassHash, CompiledClassHash>>>;

    fn deployed_contracts(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<BTreeMap<ContractAddress, ClassHash>>>;
}
