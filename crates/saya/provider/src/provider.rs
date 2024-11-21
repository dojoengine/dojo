use katana_primitives::block::{BlockNumber, SealedBlock};
use katana_primitives::state::StateUpdatesWithClasses;
use katana_rpc_types::trace::TxExecutionInfo;
use starknet::core::types::Felt;

use crate::ProviderResult;

#[async_trait::async_trait]
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait Provider {
    /// Fetches the current block number of underlying chain.
    async fn block_number(&self) -> ProviderResult<BlockNumber>;

    /// Fetches a block with it's transactions.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block to fetch.
    async fn fetch_block(&self, block_number: BlockNumber) -> ProviderResult<SealedBlock>;

    /// Fetches the state updates related to a given block.
    /// Returns the [`StateUpdatesWithClasses`] and the serialiazed
    /// state update for data availability layer.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block to fetch.
    async fn fetch_state_updates(
        &self,
        block_number: BlockNumber,
    ) -> ProviderResult<(StateUpdatesWithClasses, Vec<Felt>)>;

    /// Fetches the transactions executions info for a given block.
    /// This method returns the all the executions info for each
    /// transaction in a block.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block to fetch.
    async fn fetch_transactions_executions(
        &self,
        block_number: BlockNumber,
    ) -> ProviderResult<Vec<TxExecutionInfo>>;
}
