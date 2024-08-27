use std::sync::Arc;

use katana_executor::implementation::blockifier::blockifier::blockifier::stateful_validator::{
    StatefulValidator, StatefulValidatorError,
};
use katana_executor::implementation::blockifier::blockifier::state::cached_state::CachedState;
use katana_executor::implementation::blockifier::blockifier::transaction::errors::{
    TransactionExecutionError, TransactionFeeError, TransactionPreValidationError,
};
use katana_executor::implementation::blockifier::blockifier::transaction::transaction_execution::Transaction;
use katana_executor::implementation::blockifier::utils::{
    block_context_from_envs, to_address, to_blk_address, to_executor_tx,
};
use katana_executor::{SimulationFlag, StateProviderDb};
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash};
use katana_provider::traits::state::StateProvider;
use parking_lot::Mutex;

use super::{Error, InvalidTransactionError, ValidationOutcome, ValidationResult, Validator};
use crate::tx::PoolTransaction;

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct TxValidator {
    inner: Arc<Inner>,
}

struct Inner {
    cfg_env: CfgEnv,
    execution_flags: SimulationFlag,
    validator: Mutex<StatefulValidatorAdapter>,
    permit: Arc<Mutex<()>>,
}

impl TxValidator {
    pub fn new(
        state: Box<dyn StateProvider>,
        execution_flags: SimulationFlag,
        cfg_env: CfgEnv,
        block_env: &BlockEnv,
        permit: Arc<Mutex<()>>,
    ) -> Self {
        let validator = StatefulValidatorAdapter::new(state, block_env, &cfg_env);
        Self {
            inner: Arc::new(Inner {
                permit,
                cfg_env,
                execution_flags,
                validator: Mutex::new(validator),
            }),
        }
    }

    /// Reset the state of the validator with the given params. This method is used to update the
    /// validator's state with a new state and block env after a block is mined.
    pub fn update(&self, new_state: Box<dyn StateProvider>, block_env: &BlockEnv) {
        let mut validator = self.inner.validator.lock();

        let mut state = validator.inner.tx_executor.block_state.take().unwrap();
        state.state = StateProviderDb::new(new_state);

        *validator = StatefulValidatorAdapter::new_inner(state, block_env, &self.inner.cfg_env);
    }

    // NOTE:
    // If you check the get_nonce method of StatefulValidator in blockifier, under the hood it
    // unwraps the Option to get the state of the TransactionExecutor struct. StatefulValidator
    // guaranteees that the state will always be present so it is safe to uwnrap. However, this
    // safety is not guaranteed by TransactionExecutor itself.
    pub fn get_nonce(&self, address: ContractAddress) -> Nonce {
        let address = to_blk_address(address);
        let nonce = self.inner.validator.lock().inner.get_nonce(address).expect("state err");
        nonce.0
    }
}

#[allow(missing_debug_implementations)]
struct StatefulValidatorAdapter {
    inner: StatefulValidator<StateProviderDb<'static>>,
}

impl StatefulValidatorAdapter {
    fn new(state: Box<dyn StateProvider>, block_env: &BlockEnv, cfg_env: &CfgEnv) -> Self {
        let state = CachedState::new(StateProviderDb::new(state));
        Self::new_inner(state, block_env, cfg_env)
    }

    fn new_inner(
        state: CachedState<StateProviderDb<'static>>,
        block_env: &BlockEnv,
        cfg_env: &CfgEnv,
    ) -> Self {
        let block_context = block_context_from_envs(block_env, cfg_env);
        let inner = StatefulValidator::create(state, block_context);
        Self { inner }
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
                // Check if the transaction nonce is higher than the current account nonce,
                // if yes, dont't run its validation logic but tag it as dependent
                let account = to_blk_address(tx.sender());
                let account_nonce = self.inner.get_nonce(account).expect("state err");

                if tx.nonce() > account_nonce.0 {
                    return Ok(ValidationOutcome::Dependent {
                        current_nonce: account_nonce.0,
                        tx_nonce: tx.nonce(),
                        tx,
                    });
                }

                match self.inner.perform_validations(blockifier_tx, skip_validate, skip_fee_check) {
                    Ok(()) => Ok(ValidationOutcome::Valid(tx)),
                    Err(e) => match map_invalid_tx_err(e) {
                        Ok(error) => Ok(ValidationOutcome::Invalid { tx, error }),
                        Err(error) => Err(Error { hash: tx.hash, error }),
                    },
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
        let this = &mut *self.inner.validator.lock();

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
            self.inner.execution_flags.skip_validate || skip_validate,
            self.inner.execution_flags.skip_fee_transfer,
        )
    }
}

fn map_invalid_tx_err(
    err: StatefulValidatorError,
) -> Result<InvalidTransactionError, Box<dyn std::error::Error>> {
    match err {
        StatefulValidatorError::TransactionExecutionError(err) => match err {
            e @ TransactionExecutionError::ValidateTransactionError {
                storage_address,
                class_hash,
                ..
            } => {
                let address = to_address(storage_address);
                let class_hash = class_hash.0;
                let error = e.to_string();
                Ok(InvalidTransactionError::ValidationFailure { address, class_hash, error })
            }

            _ => Err(Box::new(err)),
        },

        StatefulValidatorError::TransactionPreValidationError(err) => match err {
            TransactionPreValidationError::InvalidNonce {
                address,
                account_nonce,
                incoming_tx_nonce,
            } => {
                let address = to_address(address);
                let current_nonce = account_nonce.0;
                let tx_nonce = incoming_tx_nonce.0;
                Ok(InvalidTransactionError::InvalidNonce { address, current_nonce, tx_nonce })
            }

            TransactionPreValidationError::TransactionFeeError(err) => match err {
                TransactionFeeError::MaxFeeExceedsBalance { max_fee, balance } => {
                    let max_fee = max_fee.0;
                    let balance = balance.into();
                    Ok(InvalidTransactionError::InsufficientFunds { max_fee, balance })
                }

                TransactionFeeError::MaxFeeTooLow { min_fee, max_fee } => {
                    let max_fee = max_fee.0;
                    let min_fee = min_fee.0;
                    Ok(InvalidTransactionError::InsufficientMaxFee { max_fee, min_fee })
                }

                _ => Err(Box::new(err)),
            },

            _ => Err(Box::new(err)),
        },

        _ => Err(Box::new(err)),
    }
}
