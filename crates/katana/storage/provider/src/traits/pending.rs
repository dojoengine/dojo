use katana_primitives::{
    env::BlockEnv,
    receipt::Receipt,
    transaction::{TxHash, TxWithHash},
};

use crate::ProviderResult;

use super::state::StateProvider;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PendingBlockProvider: Send + Sync {
    fn pending_block_env(&self) -> ProviderResult<BlockEnv>;

    fn pending_transactions(&self) -> ProviderResult<Vec<TxWithHash>>;

    fn pending_receipts(&self) -> ProviderResult<Vec<Receipt>>;

    fn pending_transaction(&self, hash: TxHash) -> ProviderResult<TxWithHash>;

    fn pending_receipt(&self, hash: TxHash) -> ProviderResult<Receipt>;

    fn pending_transaction_status(&self, hash: TxHash) -> ProviderResult<()>;

    fn pending_state(&self) -> ProviderResult<Box<dyn StateProvider>>;
}
