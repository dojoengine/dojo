use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::FieldElement;
use katana_provider::traits::block::{BlockHashProvider, BlockNumberProvider};
use katana_provider::traits::state::StateRootProvider;
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::ProviderResult;
use katana_rpc_types::state_update::{StateDiff, StateUpdate};

/// A builder for building RPC state update type.
pub struct StateUpdateBuilder<P> {
    provider: P,
    block_id: BlockHashOrNumber,
}

impl<P> StateUpdateBuilder<P> {
    pub fn new(block_id: BlockHashOrNumber, provider: P) -> Self {
        Self { provider, block_id }
    }
}

impl<P> StateUpdateBuilder<P>
where
    P: BlockHashProvider + BlockNumberProvider + StateRootProvider + StateUpdateProvider,
{
    /// Builds a state update for the given block.
    pub fn build(self) -> ProviderResult<Option<StateUpdate>> {
        let Some(block_hash) = BlockHashProvider::block_hash_by_id(&self.provider, self.block_id)?
        else {
            return Ok(None);
        };

        let new_root = StateRootProvider::state_root(&self.provider, self.block_id)?
            .expect("should exist if block exists");
        let old_root = {
            let block_num = BlockNumberProvider::block_number_by_hash(&self.provider, block_hash)?
                .expect("should exist if block exists");

            match block_num {
                0 => FieldElement::ZERO,
                _ => StateRootProvider::state_root(&self.provider, (block_num - 1).into())?
                    .expect("should exist if not genesis"),
            }
        };

        let state_diff: StateDiff =
            StateUpdateProvider::state_update(&self.provider, self.block_id)?
                .expect("should exist if block exists")
                .into();

        Ok(Some(
            starknet::core::types::StateUpdate {
                block_hash,
                new_root,
                old_root,
                state_diff: state_diff.0,
            }
            .into(),
        ))
    }
}
