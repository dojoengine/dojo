use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::Receipt;
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{TxHash, TxWithHash};
use starknet::core::types::TransactionStatus;

use super::state::StateProvider;
use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PendingBlockProvider: Send + Sync + 'static {
    fn pending_block_env(&self) -> ProviderResult<BlockEnv>;

    fn pending_block(&self) -> ProviderResult<()>;

    fn pending_transactions(&self) -> ProviderResult<Vec<TxWithHash>>;

    fn pending_receipts(&self) -> ProviderResult<Vec<Receipt>>;

    fn pending_transaction(&self, hash: TxHash) -> ProviderResult<Option<TxWithHash>>;

    fn pending_receipt(&self, hash: TxHash) -> ProviderResult<Option<Receipt>>;

    fn pending_transaction_trace(&self, hash: TxHash) -> ProviderResult<Option<TxExecInfo>>;

    fn pending_transaction_traces(&self) -> ProviderResult<Vec<TxExecInfo>>;

    fn pending_transaction_status(&self, hash: TxHash)
    -> ProviderResult<Option<TransactionStatus>>;

    fn pending_state(&self) -> ProviderResult<Box<dyn StateProvider>>;
}
