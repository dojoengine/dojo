use std::sync::Arc;

use katana_executor::implementation::blockifier::blockifier::context::BlockContext;
use katana_executor::implementation::blockifier::blockifier::state::cached_state::CachedState;
use katana_executor::implementation::blockifier::blockifier::transaction::transaction_execution::Transaction;
use katana_executor::implementation::blockifier::utils::to_executor_tx;
use katana_executor::{
    implementation::blockifier::blockifier::blockifier::stateful_validator::StatefulValidator as BlockifierValidator,
    StateProviderDb,
};
use katana_primitives::transaction::ExecutableTxWithHash;
use parking_lot::Mutex;

use super::{Error, ValidationOutcome, ValidationResult, Validator};

pub struct StatefulValidator {
    pending_state: CachedState<StateProviderDb<'static>>,
    inner: Arc<Mutex<BlockifierValidator<StateProviderDb<'static>>>>,
}

impl StatefulValidator {
    pub fn new(state: StateProviderDb<'static>, genesis_block_context: BlockContext) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BlockifierValidator::create(
                state,
                genesis_block_context,
                Default::default(),
            ))),
        }
    }

    /// update the inner state and block context
    pub fn update_state_and_block_context(
        &self,
        state: StateProviderDb<'static>,
        block_context: BlockContext,
    ) {
        *self.inner.lock() = BlockifierValidator::create(state, block_context, Default::default());
    }

    /// Used only in the [`Validator::validate`] trait
    fn valdiate(&self, tx: ExecutableTxWithHash) -> ValidationResult<ExecutableTxWithHash> {
        match to_executor_tx(tx.clone()) {
            Transaction::AccountTransaction(blockifier_tx) => {
                match self.inner.lock().perform_validations(blockifier_tx, None) {
                    Ok(()) => Ok(ValidationOutcome::Valid(tx)),
                    Err(e) => Err(Error { hash: tx.hash, error: Box::new(e) }),
                }
            }

            // we skip validation for L1HandlerTransaction
            Transaction::L1HandlerTransaction(_) => Ok(ValidationOutcome::Valid(tx)),
        }
    }
}

impl Clone for StatefulValidator {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl Validator for StatefulValidator {
    type Transaction = ExecutableTxWithHash;

    fn validate(&self, tx: Self::Transaction) -> ValidationResult<Self::Transaction> {
        Self::valdiate(self, tx)
    }
}
