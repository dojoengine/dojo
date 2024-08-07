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
pub mod tx;
pub mod validation;

use std::collections::{BTreeMap, BinaryHeap};
use std::sync::Arc;

use futures::channel::mpsc::{channel, Receiver, Sender};
use katana_primitives::transaction::TxHash;
use ordering::PoolOrd;
use parking_lot::RwLock;
use tracing::{error, warn};
use tx::{InvalidPoolTx, PoolTransaction, TxId, ValidPoolTx};
use validation::{ValidationOutcome, Validator};

#[derive(Clone)]
pub struct TxPool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    inner: Arc<Inner<T, V, O>>,
}

struct Inner<T, V, O: PoolOrd> {
    /// list of all valid txs in the pool
    valid_txs: RwLock<BTreeMap<TxId, ValidPoolTx<T, O>>>,
    /// List of independent txs that can be included.
    ///
    /// The order of the txs in the btree is determined by the priority values.
    pending_txs: RwLock<BinaryHeap<ValidPoolTx<T, O>>>,
    /// list of all invalid (aka rejected) txs in the pool
    rejected_txs: RwLock<BTreeMap<TxHash, InvalidPoolTx<T>>>,
    /// the tx validator
    validator: V,
    /// the ordering mechanism used to order the txs in the pool
    ordering: O,
    /// listeners for incoming txs
    // TODO: add listeners for different pools
    listeners: RwLock<Vec<Sender<TxHash>>>,
}

impl<T, V, O> TxPool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    pub fn new(validator: V, ordering: O) -> Self {
        Self {
            inner: Arc::new(Inner {
                ordering,
                validator,
                listeners: Default::default(),
                valid_txs: Default::default(),
                pending_txs: Default::default(),
                rejected_txs: Default::default(),
            }),
        }
    }

    /// add tx to the pool
    ///
    /// steps that must be taken before putting tx into the pool:
    /// 1. validate using the pool's [TxValidator](crate::validation::TxValidator)
    /// 2. if valid, assign a priority value to the tx using the ordering implementation. then
    ///    insert to the all/pending tx.
    /// 3. f not valid, insert to the rejected pool
    // TODO: the API should accept raw tx type, the PoolTransaction should only be used for in the
    // pool scope.
    pub fn add_transaction(&self, tx: T) {
        let id = TxId::new(*tx.sender(), *tx.nonce());

        match self.inner.validator.validate(tx) {
            Ok(outcome) => {
                match outcome {
                    ValidationOutcome::Valid(tx) => {
                        let priority = self.inner.ordering.priority(&tx);

                        // TODO: convert the base tx into a pool tx with the priority value attached
                        let pool_tx = ValidPoolTx::new(id.clone(), tx, priority);
                        let hash = *pool_tx.tx.hash();

                        self.inner.valid_txs.write().insert(id, pool_tx.clone());
                        self.inner.pending_txs.write().push(pool_tx);
                        self.notify_listener(hash);
                    }

                    ValidationOutcome::Invalid { tx, error } => {
                        let hash = *tx.hash();
                        let tx = InvalidPoolTx::new(tx, error);

                        self.inner.rejected_txs.write().insert(hash, tx);
                        // TODO: notify listeners
                    }
                }
            }

            Err(validation::Error { hash, error }) => {
                error!(hash = format!("{hash:#x}"), %error, "Invalid transaction");
            }
        }
    }

    // takes the transactions that are ready and valid to be included in the next block,
    // from the pool.
    //
    // best transactions must be ordered by the ordering mechanism used.
    pub fn take_best_transactions(&self) -> impl Iterator<Item = ValidPoolTx<T, O>> {
        BestTransactions {
            all: self.inner.valid_txs.read().clone(),
            pending: self.inner.pending_txs.read().clone(),
        }
    }

    // check if a tx is in the pool
    pub fn contains(&self, hash: TxHash) -> bool {
        self.find(hash).is_some()
    }

    pub fn find(&self, hash: TxHash) -> Option<T> {
        todo!()
    }

    // to be used for removing transactions that have been included in a block, and no longer
    // needs to be kept around in the pool.
    //
    // should remove from all the pools.
    pub fn remove_transactions(&mut self, hashes: &[TxHash]) {
        todo!()
    }

    pub fn add_listener(&self) -> Receiver<TxHash> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.inner.listeners.write().push(tx);
        rx
    }

    /// notifies all listeners about the transaction
    fn notify_listener(&self, hash: TxHash) {
        let mut listener = self.inner.listeners.write();
        // this is basically a retain but with mut reference
        for n in (0..listener.len()).rev() {
            let mut listener_tx = listener.swap_remove(n);
            let retain = match listener_tx.try_send(hash) {
                Ok(()) => true,
                Err(e) => {
                    if e.is_full() {
                        warn!(
                            hash = ?format!("\"{hash:#x}\""),
                            "Unable to send tx notification because channel is full."
                        );
                        true
                    } else {
                        false
                    }
                }
            };

            if retain {
                listener.push(listener_tx)
            }
        }
    }
}

/// an iterator that yields transactions from the pool that can be included in a block, sorted by
/// by its priority.
struct BestTransactions<T, O: PoolOrd> {
    all: BTreeMap<TxId, ValidPoolTx<T, O>>,
    pending: BinaryHeap<ValidPoolTx<T, O>>,
}

impl<T, O> Iterator for BestTransactions<T, O>
where
    T: PoolTransaction + Clone,
    O: PoolOrd<Transaction = T>,
{
    type Item = ValidPoolTx<T, O>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tx) = self.pending.pop() {
            // check if there's a dependent tx that gets unlocked by this tx
            if let Some(depedent) = self.all.get(&tx.id.descendent()) {
                self.pending.push(depedent.clone());
            }
            Some(tx)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn add_dependent_txs_in_parallel() {}
}
