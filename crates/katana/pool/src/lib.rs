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
use tracing::{error, info, warn};
use tx::{PendingTx, PoolTransaction, TxId};
use validation::{ValidationOutcome, Validator};

/// Represents a complete transaction pool.
pub trait TransactionPool {
    type Transaction: PoolTransaction;

    type Ordering: PoolOrd<Transaction = Self::Transaction>;

    type Validator: Validator<Transaction = Self::Transaction>;

    fn add_transaction(&self, tx: Self::Transaction);

    fn pending_transactions(
        &self,
    ) -> impl Iterator<Item = PendingTx<Self::Transaction, Self::Ordering>>;

    fn contains(&self, hash: TxHash) -> bool;

    fn get(&self, hash: TxHash) -> Option<Arc<Self::Transaction>>;

    fn remove_transactions(&mut self, hashes: &[TxHash]);

    fn add_listener(&self) -> Receiver<TxHash>;

    /// get the total number of transactions in the pool.
    fn size(&self) -> usize;
}

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
    /// List of all valid txs mapped by their hash.
    valid_ids_by_hash: RwLock<BTreeMap<TxHash, TxId>>,

    /// List of all valid txs in the pool
    valid_txs: RwLock<BTreeMap<TxId, PendingTx<T, O>>>,

    /// List of independent txs that can be included. A subset of the valid txs.
    ///
    /// The txs are sorted by the priority values.
    pending_txs: RwLock<BinaryHeap<PendingTx<T, O>>>,

    /// list of all invalid (aka rejected) txs in the pool
    // TODO: add timeout eviction policy
    rejected_txs: RwLock<BTreeMap<TxHash, Arc<T>>>,

    /// listeners for incoming txs
    // TODO: add listeners for different pools
    listeners: RwLock<Vec<Sender<TxHash>>>,

    /// the tx validator
    validator: V,

    /// the ordering mechanism used to order the txs in the pool
    ordering: O,
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
                valid_ids_by_hash: Default::default(),
            }),
        }
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

impl<T, V, O> TransactionPool for TxPool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    type Transaction = T;
    type Validator = V;
    type Ordering = O;

    /// add tx to the pool
    ///
    /// steps that must be taken before putting tx into the pool:
    /// 1. validate using the pool's [TxValidator](crate::validation::TxValidator)
    /// 2. if valid, assign a priority value to the tx using the ordering implementation. then
    ///    insert to the all/pending tx.
    /// 3. f not valid, insert to the rejected pool
    // TODO: the API should accept raw tx type, the PoolTransaction should only be used for in the
    // pool scope.
    fn add_transaction(&self, tx: T) {
        let id = TxId::new(tx.sender(), tx.nonce());

        match self.inner.validator.validate(tx) {
            Ok(outcome) => {
                let hash = match outcome {
                    ValidationOutcome::Valid(tx) => {
                        let priority = self.inner.ordering.priority(&tx);

                        // TODO: convert the base tx into a pool tx with the priority value attached
                        let pool_tx = PendingTx::new(id.clone(), tx, priority);
                        let hash = pool_tx.tx.hash();

                        self.inner.valid_txs.write().insert(id, pool_tx.clone());
                        self.inner.pending_txs.write().push(pool_tx);
                        self.notify_listener(hash);
                        hash
                    }

                    ValidationOutcome::Invalid { tx, .. } => {
                        let hash = tx.hash();
                        self.inner.rejected_txs.write().insert(hash, Arc::new(tx));
                        // TODO: notify listeners
                        hash
                    }
                };

                info!(hash = format!("\"{hash:#x}\""), "Transaction added to pool");
            }

            Err(error @ validation::Error { hash, .. }) => {
                error!(hash = format!("{hash:#x}"), %error, "Failed to validate transaction");
            }
        }
    }

    // takes the transactions that are ready and valid to be included in the next block,
    // from the pool.
    //
    // best transactions must be ordered by the ordering mechanism used.
    fn pending_transactions(&self) -> impl Iterator<Item = PendingTx<T, O>> {
        PendingTransactions {
            all: self.inner.valid_txs.read().clone(),
            pending: self.inner.pending_txs.read().clone(),
        }
    }

    // check if a tx is in the pool
    fn contains(&self, hash: TxHash) -> bool {
        self.get(hash).is_some()
    }

    fn get(&self, hash: TxHash) -> Option<Arc<T>> {
        // check in the valid list
        if let Some(tx) = self
            .inner
            .valid_ids_by_hash
            .read()
            .get(&hash)
            .and_then(|id| self.inner.valid_txs.read().get(id).map(|tx| tx.tx.clone()))
        {
            return Some(tx);
        }

        // if not found, check in the rejected list
        if let Some(tx) = self.inner.rejected_txs.read().get(&hash).map(Arc::clone) {
            return Some(tx);
        }

        None
    }

    // to be used for removing transactions that have been included in a block, and no longer
    // needs to be kept around in the pool.
    //
    // should remove from all the pools.
    fn remove_transactions(&mut self, hashes: &[TxHash]) {
        todo!()
    }

    fn add_listener(&self) -> Receiver<TxHash> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.inner.listeners.write().push(tx);
        rx
    }

    fn size(&self) -> usize {
        self.inner.valid_txs.read().len() + self.inner.rejected_txs.read().len()
    }
}

/// an iterator that yields transactions from the pool that can be included in a block, sorted by
/// by its priority.
struct PendingTransactions<T, O: PoolOrd> {
    all: BTreeMap<TxId, PendingTx<T, O>>,
    pending: BinaryHeap<PendingTx<T, O>>,
}

impl<T, O> Iterator for PendingTransactions<T, O>
where
    T: PoolTransaction + Clone,
    O: PoolOrd<Transaction = T>,
{
    type Item = PendingTx<T, O>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tx) = self.pending.pop() {
            // check if there's a dependent tx that gets unlocked by this tx
            if let Some(tx) = self.all.get(&tx.id.descendent()) {
                // insert the unlocked tx to the pending pool
                self.pending.push(tx.clone());
            }
            Some(tx)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ordering::Fcfs, tx::PoolTransaction, validation::NoopValidator, TransactionPool, TxPool,
    };
    use katana_primitives::{
        contract::{ContractAddress, Nonce},
        FieldElement,
    };
    use rand::Rng;

    fn random_bytes<const SIZE: usize>() -> [u8; SIZE] {
        let mut bytes = [0u8; SIZE];
        rand::thread_rng().fill(&mut bytes[..]);
        bytes
    }

    #[derive(Clone)]
    struct PoolTx;

    impl PoolTransaction for PoolTx {
        fn hash(&self) -> TxHash {
            TxHash::from_bytes_be(&random_bytes::<32>())
        }

        fn max_fee(&self) -> u64 {
            rand::thread_rng().gen()
        }

        fn nonce(&self) -> Nonce {
            Nonce::from_bytes_be(&random_bytes::<32>())
        }

        fn sender(&self) -> katana_primitives::contract::ContractAddress {
            let felt = FieldElement::from_bytes_be(&random_bytes::<32>());
            ContractAddress::from(felt)
        }

        fn tip(&self) -> u64 {
            rand::thread_rng().gen()
        }
    }

    type MockTxPool<V, O> = TxPool<PoolTx, V, O>;

    #[test]
    fn add_txs() {
        let pool = MockTxPool::new(NoopValidator::new(), Fcfs::new());
        pool.add_transaction(PoolTx);
        pool.add_transaction(PoolTx);
        pool.add_transaction(PoolTx);
        pool.add_transaction(PoolTx);
        pool.add_transaction(PoolTx);
        pool.add_transaction(PoolTx);
        pool.add_transaction(PoolTx);
        pool.add_transaction(PoolTx);

        assert_eq!(pool.size(), 8);
    }
}
