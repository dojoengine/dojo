use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::Receipt;
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{TxHash, TxWithHash};
use starknet::core::types::TransactionStatus;

use super::state::StateProvider;
use crate::ProviderResult;

/// A provider for pending block data ie., header, transactions, receipts, traces (if any).
//
// In the context of a full node, where the node doesn't produce the blocks itself, how it can provide
// the pending block data could be from a remote sequencer or feeder gateway. But, if the node itself
// is a sequencer, it can provide the pending block data from its own local state. So, the main motivation
// for this trait is to provide a common interface for both cases.
//
// TODO: Maybe more to rpc crate as this is mainly gonna be used in the rpc side.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PendingBlockProvider: Send + Sync + 'static {
    fn pending_block_env(&self) -> ProviderResult<BlockEnv>;

    fn pending_block(&self) -> ProviderResult<()>;

    /// Returns all the transactions that are currently in the pending block.
    fn pending_transactions(&self) -> ProviderResult<Vec<TxWithHash>>;

    /// Returns all the receipts that are currently in the pending block.
    fn pending_receipts(&self) -> ProviderResult<Vec<Receipt>>;

    /// Returns all the transaction traces that are currently in the pending block.
    fn pending_transaction_traces(&self) -> ProviderResult<Vec<TxExecInfo>>;

    /// Returns a transaction in the pending block by its hash.
    fn pending_transaction(&self, hash: TxHash) -> ProviderResult<Option<TxWithHash>>;

    /// Returns a receipt in the pending block by its hash.
    fn pending_receipt(&self, hash: TxHash) -> ProviderResult<Option<Receipt>>;

    /// Returns a transaction trace in the pending block by its hash.
    fn pending_transaction_trace(&self, hash: TxHash) -> ProviderResult<Option<TxExecInfo>>;

    fn pending_transaction_status(&self, hash: TxHash)
        -> ProviderResult<Option<TransactionStatus>>;

    /// Returns a [`StateProvider`] for the pending state.
    fn pending_state(&self) -> ProviderResult<Box<dyn StateProvider>>;
}
