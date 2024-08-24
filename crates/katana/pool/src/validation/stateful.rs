use std::sync::Arc;

use katana_executor::implementation::blockifier::blockifier::blockifier::stateful_validator::{
    StatefulValidator as BlockifierValidator, StatefulValidatorError,
};
use katana_executor::implementation::blockifier::blockifier::state::cached_state::CachedState;
use katana_executor::implementation::blockifier::blockifier::transaction::transaction_execution::Transaction;
use katana_executor::implementation::blockifier::utils::{block_context_from_envs, to_executor_tx};
use katana_executor::StateProviderDb;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_provider::traits::state::StateProvider;
use parking_lot::Mutex;

use super::{Error, ValidationOutcome, ValidationResult, Validator};

#[derive(Clone)]
pub struct TxValidator(Arc<Mutex<StatefulValidatorAdapter>>);

impl TxValidator {
    pub fn new(state: Box<dyn StateProvider>, block_env: &BlockEnv, cfg_env: &CfgEnv) -> Self {
        let inner = StatefulValidatorAdapter::new(state, block_env, cfg_env);
        Self(Arc::new(Mutex::new(inner)))
    }

    pub fn reset(&self, state: Box<dyn StateProvider>, block_env: &BlockEnv, cfg_env: &CfgEnv) {
        *self.0.lock() = StatefulValidatorAdapter::new(state, block_env, cfg_env);
    }
}

pub struct StatefulValidatorAdapter {
    inner: BlockifierValidator<StateProviderDb<'static>>,
}

// pool state (only stores storage changes during tx validation + nonce updates) -> pending state
// upon every mined block, reset the pool state to the new pending state after the block is mined

impl StatefulValidatorAdapter {
    pub fn new(
        state: Box<dyn StateProvider>,
        block_env: &BlockEnv,
        cfg_env: &CfgEnv,
    ) -> StatefulValidatorAdapter {
        Self { inner: Self::new_inner(state, block_env, cfg_env) }
    }

    fn new_inner(
        state: Box<dyn StateProvider>,
        block_env: &BlockEnv,
        cfg_env: &CfgEnv,
    ) -> BlockifierValidator<StateProviderDb<'static>> {
        let state = CachedState::new(StateProviderDb::new(state));
        let block_context = block_context_from_envs(&block_env, &cfg_env);
        BlockifierValidator::create(state, block_context, Default::default())
    }

    /// Used only in the [`Validator::validate`] trait
    fn validate(&mut self, tx: ExecutableTxWithHash) -> ValidationResult<ExecutableTxWithHash> {
        match to_executor_tx(tx.clone()) {
            Transaction::AccountTransaction(blockifier_tx) => {
                match self.inner.perform_validations(blockifier_tx, None) {
                    Ok(()) => Ok(ValidationOutcome::Valid(tx)),
                    // TODO: implement from<statefulvalidatorerror> for invalidtransactionerror
                    Err(e) => Err(Error { hash: tx.hash, error: Box::new(e) }),
                }
            }

            // we skip validation for L1HandlerTransaction
            Transaction::L1HandlerTransaction(_) => Ok(ValidationOutcome::Valid(tx)),
        }
    }
}

impl Validator for TxValidator {
    type Transaction = ExecutableTxWithHash;

    fn validate(&self, tx: Self::Transaction) -> ValidationResult<Self::Transaction> {
        let this = &mut *self.0.lock();
        StatefulValidatorAdapter::validate(this, tx)
    }
}
