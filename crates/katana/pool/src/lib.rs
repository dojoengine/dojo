#![cfg_attr(not(test), warn(unused_crate_dependencies))]

pub mod ordering;
pub mod pending;
pub mod pool;
pub mod subscription;
pub mod tx;
pub mod validation;

use std::sync::Arc;

use futures::channel::mpsc::Receiver;
use katana_primitives::transaction::{ExecutableTxWithHash, TxHash};
use ordering::{FiFo, PoolOrd};
use pending::PendingTransactions;
use pool::Pool;
use tx::PoolTransaction;
use validation::error::InvalidTransactionError;
use validation::stateful::TxValidator;
use validation::Validator;

/// Katana default transacstion pool type.
pub type TxPool = Pool<ExecutableTxWithHash, TxValidator, FiFo<ExecutableTxWithHash>>;

pub type PoolResult<T> = Result<T, PoolError>;

#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(Box<InvalidTransactionError>),
    #[error("Internal error: {0}")]
    Internal(Box<dyn std::error::Error>),
}

/// Represents a complete transaction pool.
pub trait TransactionPool {
    /// The pool's transaction type.
    type Transaction: PoolTransaction;

    /// The ordering mechanism to use. This is used to determine
    /// how transactions are being ordered within the pool.
    type Ordering: PoolOrd<Transaction = Self::Transaction>;

    /// Transaction validation before adding to the pool.
    type Validator: Validator<Transaction = Self::Transaction>;

    /// Add a new transaction to the pool.
    fn add_transaction(&self, tx: Self::Transaction) -> PoolResult<TxHash>;

    /// Returns a [`Stream`](futures::Stream) which yields pending transactions - transactions that
    /// can be executed - from the pool.
    fn pending_transactions(&self) -> PendingTransactions<Self::Transaction, Self::Ordering>;

    /// Check if the pool contains a transaction with the given hash.
    fn contains(&self, hash: TxHash) -> bool;

    /// Get a transaction from the pool by its hash.
    fn get(&self, hash: TxHash) -> Option<Arc<Self::Transaction>>;

    fn add_listener(&self) -> Receiver<TxHash>;

    /// Removes a list of transactions from the pool according to their hashes.
    fn remove_transactions(&self, hashes: &[TxHash]);

    /// Get the total number of transactions in the pool.
    fn size(&self) -> usize;

    /// Get a reference to the pool's validator.
    fn validator(&self) -> &Self::Validator;
}
