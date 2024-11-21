//! Blockchain fetched from Katana.

use katana_primitives::block::{BlockHashOrNumber, BlockIdOrTag, BlockTag, SealedBlockWithStatus};
use katana_primitives::state::StateUpdatesWithClasses;
use katana_provider::providers::db::DbProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::contract::ContractClassWriter;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::{
    StateFactoryProvider, StateProvider, StateRootProvider, StateWriter,
};
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider, TransactionsProviderExt,
};
use katana_provider::BlockchainProvider;

use crate::error::{Error as SayaError, SayaResult};

pub trait Database:
    BlockProvider
    + BlockWriter
    + TransactionProvider
    + TransactionStatusProvider
    + TransactionsProviderExt
    + ReceiptProvider
    + StateUpdateProvider
    + StateRootProvider
    + StateWriter
    + ContractClassWriter
    + StateFactoryProvider
    + BlockEnvProvider
    + 'static
    + Send
    + Sync
{
}

impl<T> Database for T where
    T: BlockProvider
        + BlockWriter
        + TransactionProvider
        + TransactionStatusProvider
        + TransactionsProviderExt
        + ReceiptProvider
        + StateUpdateProvider
        + StateRootProvider
        + StateWriter
        + ContractClassWriter
        + StateFactoryProvider
        + BlockEnvProvider
        + 'static
        + Send
        + Sync
{
}

/// Represents the whole blockchain fetched from Katana.
#[allow(missing_debug_implementations)]
pub struct Blockchain {
    inner: BlockchainProvider<Box<dyn Database>>,
}

impl Default for Blockchain {
    fn default() -> Self {
        Self::new()
    }
}

impl Blockchain {
    /// Initializes a new instance of [`Blockchain`].
    pub fn new() -> Self {
        Self { inner: BlockchainProvider::new(Box::new(DbProvider::new_ephemeral())) }
    }

    /// Returns the internal provider.
    pub fn provider(&self) -> &BlockchainProvider<Box<dyn Database>> {
        &self.inner
    }

    /// Retrieves historical state for the given block.
    ///
    /// # Arguments
    ///
    /// * `block_id` - The block id at which the state must be retrieved.
    pub fn state(&self, block_id: &BlockIdOrTag) -> SayaResult<Box<dyn StateProvider>> {
        let provider = self.provider();

        match block_id {
            BlockIdOrTag::Tag(BlockTag::Latest) => {
                let state = StateFactoryProvider::latest(provider)?;
                Ok(state)
            }

            BlockIdOrTag::Hash(hash) => {
                StateFactoryProvider::historical(provider, BlockHashOrNumber::Hash(*hash))?
                    .ok_or(SayaError::BlockNotFound(*block_id))
            }

            BlockIdOrTag::Number(num) => {
                StateFactoryProvider::historical(provider, BlockHashOrNumber::Num(*num))?
                    .ok_or(SayaError::BlockNotFound(*block_id))
            }

            BlockIdOrTag::Tag(BlockTag::Pending) => {
                panic!("Pending block is not supported");
            }
        }
    }

    /// Updates the [`Blockchain`] internal state adding the given [`SealedBlockWithStatus`]
    /// and the associated [`StateUpdatesWithDeclaredClasses`].
    ///
    /// Currently receipts are ignored.
    ///
    /// # Arguments
    ///
    /// * `block` - The block to add.
    /// * `states` - The state updates associated with the block.
    pub fn update_state_with_block(
        &mut self,
        block: SealedBlockWithStatus,
        states: StateUpdatesWithClasses,
    ) -> SayaResult<()> {
        let provider = self.provider();
        // Receipts are not supported currently. We may need them if some
        // information about the transaction is missing.
        let receipts = vec![];

        Ok(provider.insert_block_with_states_and_receipts(block, states, receipts, vec![])?)
    }
}
