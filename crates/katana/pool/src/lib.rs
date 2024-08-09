#![cfg_attr(not(test), warn(unused_crate_dependencies))]

// - txs of the same sender must be ordered by nonce (so needs some form of tx ordering mechanism)
// - gets notification something happen to a transaction (new, removed, executed, etc).
// - being able to send transactions (that are valid) but with incremented nonce. allowing for
//   sending bunch of txs at once without waiting
// for the previous one to be executed first (to prevent nonce collision).
// - subscribe to a particular tx and gets notified when something happened to it (optional).

// - stateful validator running on top of the tx pool that validate incoming tx and puts in the pool
//   if valid. (Adding a pre-validation stage would mean we'd be running the validation stage twice)
// - valid txs must be valid against all the txs in the pool as well, not just the one in the
//   pending block

// - use a cache with timeout eviction policy for the rejected txs pool (txs that gets rejected by
//   the TxValidator trait)

// ### State Changes
//
// Once a new block is mined, the pool needs to be updated with a changeset in order to:
//
//   - remove mined transactions
//

pub mod ordering;
pub mod pool;
pub mod tx;
pub mod validation;

use std::sync::Arc;

use futures::channel::mpsc::Receiver;
use katana_primitives::transaction::{ExecutableTxWithHash, TxHash};
use ordering::{Fcfs, PoolOrd};
use pool::Pool;
use tx::{PendingTx, PoolTransaction};
use validation::{NoopValidator, Validator};

/// Katana default transacstion pool type.
pub type TxPool =
    Pool<ExecutableTxWithHash, NoopValidator<ExecutableTxWithHash>, Fcfs<ExecutableTxWithHash>>;

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
    fn add_transaction(&self, tx: Self::Transaction);

    fn pending_transactions(
        &self,
    ) -> impl Iterator<Item = PendingTx<Self::Transaction, Self::Ordering>>;

    /// Check if the pool contains a transaction with the given hash.
    fn contains(&self, hash: TxHash) -> bool;

    /// Get a transaction from the pool by its hash.
    fn get(&self, hash: TxHash) -> Option<Arc<Self::Transaction>>;

    /// Remove a transaction from the pool.
    fn remove_transactions(&self, hashes: &[TxHash]);

    fn add_listener(&self) -> Receiver<TxHash>;

    /// Get the total number of transactions in the pool.
    fn size(&self) -> usize;

    /// Get a reference to the pool's validator.
    fn validator(&self) -> &Self::Validator;
}
