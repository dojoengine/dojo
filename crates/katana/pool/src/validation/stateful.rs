use std::sync::Arc;

use katana_executor::implementation::blockifier::blockifier::blockifier::stateful_validator::StatefulValidator as BlockifierValidator;
use katana_executor::implementation::blockifier::blockifier::context::BlockContext;
use katana_executor::implementation::blockifier::blockifier::state::cached_state::CachedState;
use katana_executor::implementation::blockifier::blockifier::state::state_api::StateReader;
use katana_executor::implementation::blockifier::blockifier::transaction::transaction_execution::Transaction;
use katana_executor::implementation::blockifier::utils::to_executor_tx;
use katana_primitives::transaction::ExecutableTxWithHash;
use parking_lot::Mutex;

use super::{Error, ValidationOutcome, ValidationResult};

pub struct StatefulValidator<S: StateReader> {
    inner: Arc<Mutex<BlockifierValidator<S>>>,
}

impl<S: StateReader> StatefulValidator<S> {
    pub fn new(state: CachedState<S>, genesis_block_context: BlockContext) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BlockifierValidator::create(
                state,
                genesis_block_context,
                Default::default(),
            ))),
        }
    }

    pub fn validate(&mut self, tx: ExecutableTxWithHash) -> ValidationResult<ExecutableTxWithHash> {
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

    pub fn update_state_and_block_context(&mut self) {
        todo!()
    }
}

impl<S: StateReader> Clone for StatefulValidator<S> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
