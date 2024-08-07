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

// - use a time-based cache for a rejected txs pool (txs that gets rejected by the TxValidator trait)

// ### State Changes
//
// Once a new block is mined, the pool needs to be updated with a changeset in order to:
//
//   - remove mined transactions
//

pub mod tx;
pub mod ordering;
pub mod validation;

use std::{collections::{BTreeMap, BinaryHeap}, sync::Arc};

use tracing::error;
use katana_primitives::transaction::TxHash;
use ordering::PoolOrd;
use parking_lot::RwLock;
use tx::{PoolTransaction, TxId};
use validation::{ValidationOutcome, Validator};

#[derive(Debug, Clone)]
pub struct TxPool<T, V, O> {
    inner: Arc<Inner<T, V, O>>,
}

#[derive(Debug)]
struct Inner<T, V, O> {
    /// list of all valid txs in the pool
    valid_txs: RwLock<BTreeMap<TxId, T>>,
    /// List of independent txs that can be included.
    ///
    /// The order of the txs in the btree is determined by the priority values.
    pending_txs: RwLock<BinaryHeap<T>>,
    /// list of all invalid (aka rejected) txs in the pool
    rejected_txs: RwLock<BTreeMap<TxId, T>>,
    /// the tx validator
    validator: V,
    /// the ordering mechanism used to order the txs in the pool
    ordering: O,
}

impl<T, V, O> TxPool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Tx = T>,
    O: PoolOrd<Tx = T>,
{
    /// add tx to the pool
    ///
    /// steps that must be taken before putting tx into the pool:
    /// 1. validate using the pool's [TxValidator](crate::validation::TxValidator)
    /// 2. if valid, assign a priority value to the tx using the ordering implementation. then insert to the all/pending tx.
    /// 3. f not valid, insert to the rejected pool
    /// 
    // TODO: the API should accept raw tx type, the PoolTransaction should only be used for in the 
    // pool scope.
    pub fn add_transaction(&self, tx: T) {
        match self.inner.validator.validate(tx) {
            Ok(outcome) => {
                match outcome {
                    ValidationOutcome::Valid { tx, .. } => {
                        let priority = self.inner.ordering.priority(&tx);

                        // TODO: convert the base tx into a pool tx with the priority value attached

                        // self.all_txs.write().insert(tx.id().clone(), tx.clone());
                        // self.pending_txs.write().push(tx);
                    },
                    ValidationOutcome::Invalid { tx, .. } => {
                        todo!("insert into the rejected pool");
                    },
                }

                // TODO: notify listeners
            },

            Err(e) => {
                error!(e);
            }
        }
    }

    // returns transactions that are ready and valid to be included in the next block.
    //
    // best transactions must be ordered by the ordering mechanism used.
    pub fn best_transactions(&self) -> impl Iterator<Item = T> {
        BestTxs {
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
}

/// an iterator that yields transactions from the pool that can be included in a block, sorted by
/// by its priority.
#[derive(Debug)]
struct BestTxs<T> {
    all: BTreeMap<TxId, T>,
    pending: BinaryHeap<T>,
}

impl<T> Iterator for BestTxs<T>
where
    T: PoolTransaction + Clone,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tx) = self.pending.pop() {
            // check if there's a dependent tx that gets unlocked by this tx
            if let Some(depedent) = self.all.get(&tx.id().descendent()) {
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
