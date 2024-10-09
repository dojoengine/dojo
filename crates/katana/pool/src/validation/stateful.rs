use std::collections::HashMap;
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
    block_context_from_envs, to_address, to_executor_tx,
};
use katana_executor::{SimulationFlag, StateProviderDb};
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash};
use katana_primitives::Felt;
use katana_provider::error::ProviderError;
use katana_provider::traits::state::StateProvider;
use parking_lot::Mutex;

use super::{Error, InvalidTransactionError, ValidationOutcome, ValidationResult, Validator};
use crate::tx::PoolTransaction;

#[derive(Debug, Clone)]
pub struct TxValidator {
    inner: Arc<Mutex<Inner>>,
    permit: Arc<Mutex<()>>,
}

#[derive(Debug)]
struct Inner {
    // execution context
    cfg_env: CfgEnv,
    block_env: BlockEnv,
    execution_flags: SimulationFlag,
    state: Arc<Box<dyn StateProvider>>,

    pool_nonces: HashMap<ContractAddress, Nonce>,
}

impl TxValidator {
    pub fn new(
        state: Box<dyn StateProvider>,
        execution_flags: SimulationFlag,
        cfg_env: CfgEnv,
        block_env: BlockEnv,
        permit: Arc<Mutex<()>>,
    ) -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            cfg_env,
            block_env,
            execution_flags,
            state: Arc::new(state),
            pool_nonces: HashMap::new(),
        }));
        Self { permit, inner }
    }

    /// Reset the state of the validator with the given params. This method is used to update the
    /// validator's state with a new state and block env after a block is mined.
    pub fn update(&self, new_state: Box<dyn StateProvider>, block_env: BlockEnv) {
        let mut this = self.inner.lock();
        this.block_env = block_env;
        this.state = Arc::new(new_state);
    }

    // NOTE:
    // If you check the get_nonce method of StatefulValidator in blockifier, under the hood it
    // unwraps the Option to get the state of the TransactionExecutor struct. StatefulValidator
    // guaranteees that the state will always be present so it is safe to uwnrap. However, this
    // safety is not guaranteed by TransactionExecutor itself.
    pub fn pool_nonce(&self, address: ContractAddress) -> Result<Option<Nonce>, ProviderError> {
        let this = self.inner.lock();
        match this.pool_nonces.get(&address) {
            Some(nonce) => Ok(Some(*nonce)),
            None => Ok(this.state.nonce(address)?),
        }
    }
}

impl Inner {
    // Prepare the stateful validator with the current state and block env to be used
    // for transaction validation.
    fn prepare(&self) -> StatefulValidator<StateProviderDb<'static>> {
        let state = Box::new(self.state.clone());
        let cached_state = CachedState::new(StateProviderDb::new(state));
        let context = block_context_from_envs(&self.block_env, &self.cfg_env);
        StatefulValidator::create(cached_state, context)
    }
}

impl Validator for TxValidator {
    type Transaction = ExecutableTxWithHash;

    fn validate(&self, tx: Self::Transaction) -> ValidationResult<Self::Transaction> {
        let _permit = self.permit.lock();
        let mut this = self.inner.lock();

        let tx_nonce = tx.nonce();
        let address = tx.sender();

        // Get the current nonce of the account from the pool or the state
        let current_nonce = if let Some(nonce) = this.pool_nonces.get(&address) {
            *nonce
        } else {
            this.state.nonce(address).unwrap().unwrap_or_default()
        };

        // Check if the transaction nonce is higher than the current account nonce,
        // if yes, dont't run its validation logic and tag it as a dependent tx.
        if tx_nonce > current_nonce {
            return Ok(ValidationOutcome::Dependent { current_nonce, tx_nonce, tx });
        }

        // Check if validation of an invoke transaction should be skipped due to deploy_account not
        // being proccessed yet. This feature is used to improve UX for users sending
        // deploy_account + invoke at once.
        let skip_validate = match tx.transaction {
            // we skip validation for invoke tx with nonce 1 and nonce 0 in the state, this
            ExecutableTx::DeployAccount(_) | ExecutableTx::Declare(_) => false,
            // we skip validation for invoke tx with nonce 1 and nonce 0 in the state, this
            _ => tx.nonce() == Nonce::ONE && current_nonce == Nonce::ZERO,
        };

        // prepare a stateful validator and validate the transaction
        let result = validate(
            this.prepare(),
            tx,
            this.execution_flags.skip_validate || skip_validate,
            this.execution_flags.skip_fee_transfer,
        );

        match result {
            res @ Ok(ValidationOutcome::Valid { .. }) => {
                // update the nonce of the account in the pool only for valid tx
                let updated_nonce = current_nonce + Felt::ONE;
                this.pool_nonces.insert(address, updated_nonce);
                res
            }
            _ => result,
        }
    }
}

// perform validation on the pool transaction using the provided stateful validator
fn validate(
    mut validator: StatefulValidator<StateProviderDb<'static>>,
    pool_tx: ExecutableTxWithHash,
    skip_validate: bool,
    skip_fee_check: bool,
) -> ValidationResult<ExecutableTxWithHash> {
    match to_executor_tx(pool_tx.clone()) {
        Transaction::AccountTransaction(tx) => {
            match validator.perform_validations(tx, skip_validate, skip_fee_check) {
                Ok(()) => Ok(ValidationOutcome::Valid(pool_tx)),
                Err(e) => match map_invalid_tx_err(e) {
                    Ok(error) => Ok(ValidationOutcome::Invalid { tx: pool_tx, error }),
                    Err(error) => Err(Error { hash: pool_tx.hash, error }),
                },
            }
        }

        // we skip validation for L1HandlerTransaction
        Transaction::L1HandlerTransaction(_) => Ok(ValidationOutcome::Valid(pool_tx)),
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
