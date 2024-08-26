use std::sync::Arc;

use katana_executor::implementation::blockifier::blockifier::blockifier::stateful_validator::{
    StatefulValidator as BlockifierValidator, StatefulValidatorError,
};
use katana_executor::implementation::blockifier::blockifier::state::cached_state::CachedState;
use katana_executor::implementation::blockifier::blockifier::state::errors::StateError;
use katana_executor::implementation::blockifier::blockifier::transaction::errors::{
    TransactionExecutionError, TransactionFeeError, TransactionPreValidationError,
};
use katana_executor::implementation::blockifier::blockifier::transaction::transaction_execution::Transaction;
use katana_executor::implementation::blockifier::utils::{
    block_context_from_envs, to_blk_address, to_executor_tx,
};
use katana_executor::{SimulationFlag, StateProviderDb};
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash};
use katana_provider::traits::state::StateProvider;
use parking_lot::Mutex;

use super::{Error, InvalidTransactionError, ValidationOutcome, ValidationResult, Validator};
use crate::tx::PoolTransaction;

#[derive(Clone)]
pub struct TxValidator {
    cfg_env: CfgEnv,
    execution_flags: SimulationFlag,
    validator: Arc<Mutex<StatefulValidatorAdapter>>,
}

impl TxValidator {
    pub fn new(
        state: Box<dyn StateProvider>,
        execution_flags: SimulationFlag,
        cfg_env: CfgEnv,
        block_env: &BlockEnv,
    ) -> Self {
        let inner = StatefulValidatorAdapter::new(state, block_env, &cfg_env);
        Self { cfg_env, execution_flags, validator: Arc::new(Mutex::new(inner)) }
    }

    /// Reset the state of the validator with the given params. This method is used to update the
    /// validator's state with a new state and block env after a block is mined.
    pub fn update(&self, state: Box<dyn StateProvider>, block_env: &BlockEnv) {
        let updated = StatefulValidatorAdapter::new(state, block_env, &self.cfg_env);
        *self.validator.lock() = updated;
    }

    // NOTE:
    // If you check the get_nonce method of StatefulValidator in blockifier, under the hood it
    // unwraps the Option to get the state of the TransactionExecutor struct. StatefulValidator
    // guaranteees that the state will always be present so it is safe to uwnrap. However, this
    // safety is not guaranteed by TransactionExecutor itself.
    pub fn get_nonce(&self, address: ContractAddress) -> Nonce {
        let address = to_blk_address(address);
        let nonce = self.validator.lock().inner.get_nonce(address).expect("state err");
        nonce.0
    }
}

pub struct StatefulValidatorAdapter {
    inner: BlockifierValidator<StateProviderDb<'static>>,
}

impl StatefulValidatorAdapter {
    pub fn new(
        state: Box<dyn StateProvider>,
        block_env: &BlockEnv,
        cfg_env: &CfgEnv,
    ) -> StatefulValidatorAdapter {
        let inner = Self::new_inner(state, block_env, cfg_env);
        Self { inner }
    }

    fn new_inner(
        state: Box<dyn StateProvider>,
        block_env: &BlockEnv,
        cfg_env: &CfgEnv,
    ) -> BlockifierValidator<StateProviderDb<'static>> {
        let state = CachedState::new(StateProviderDb::new(state));
        let block_context = block_context_from_envs(&block_env, &cfg_env);
        BlockifierValidator::create(state, block_context)
    }

    /// Used only in the [`Validator::validate`] trait
    fn validate(
        &mut self,
        tx: ExecutableTxWithHash,
        skip_validate: bool,
        skip_fee_check: bool,
    ) -> ValidationResult<ExecutableTxWithHash> {
        match to_executor_tx(tx.clone()) {
            Transaction::AccountTransaction(blockifier_tx) => {
                match self.inner.perform_validations(blockifier_tx, skip_validate, skip_fee_check) {
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
        let this = &mut *self.validator.lock();

        // Check if validation of an invoke transaction should be skipped due to deploy_account not
        // being proccessed yet. This feature is used to improve UX for users sending
        // deploy_account + invoke at once.
        let skip_validate = match tx.transaction {
            // we skip validation for invoke tx with nonce 1 and nonce 0 in the state, this
            ExecutableTx::DeployAccount(_) | ExecutableTx::Declare(_) => false,

            // we skip validation for invoke tx with nonce 1 and nonce 0 in the state, this
            _ => {
                let address = to_blk_address(tx.sender());
                let account_nonce = this.inner.get_nonce(address).expect("state err");
                tx.nonce() == Nonce::ONE && account_nonce.0 == Nonce::ZERO
            }
        };

        StatefulValidatorAdapter::validate(
            this,
            tx,
            self.execution_flags.skip_validate || skip_validate,
            self.execution_flags.skip_fee_transfer,
        )
    }
}

impl From<StatefulValidatorError> for InvalidTransactionError {
    fn from(value: StatefulValidatorError) -> Self {
        match value {
            StatefulValidatorError::StateError(err) => match err {
                _ => panic!("Unhandled StateError: {:?}", err),
            },

            StatefulValidatorError::TransactionExecutionError(err) => match err {
                TransactionExecutionError::ValidateTransactionError { .. } => {
                    Self::InvalidSignature { error }
                }

                _ => panic!("Unhandled TransactionExecutionError: {:?}", err),
            },

            StatefulValidatorError::TransactionPreValidationError(err) => match err {
                TransactionPreValidationError::InvalidNonce {
                    address,
                    account_nonce,
                    incoming_tx_nonce,
                } => Self::InvalidNonce { address, account_nonce, tx_nonce: incoming_tx_nonce },

                TransactionPreValidationError::TransactionFeeError(fee_err) => match fee_err {
                    TransactionFeeError::MaxFeeExceedsBalance { max_fee, balance } => {
                        Self::InsufficientBalance { max_fee, balance }
                    }

                    TransactionFeeError::MaxFeeTooLow { min_fee, max_fee } => {
                        Self::InsufficientMaxFee { max_fee, min_fee }
                    }

                    _ => panic!("Unhandled TransactionFeeError: {:?}", fee_err),
                },

                _ => panic!("Unhandled TransactionPreValidationError: {:?}", err),
            },

            StatefulValidatorError::TransactionExecutorError(err) => {
                panic!("Unhandled TransactionExecutorError: {:?}", err)
            }
        }
    }
}
