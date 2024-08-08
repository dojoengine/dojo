use std::collections::{BTreeMap, BinaryHeap, HashMap};
use std::sync::Arc;

use futures::channel::mpsc::{channel, Receiver, Sender};
use katana_primitives::transaction::TxHash;
use parking_lot::RwLock;
use tracing::{error, info, warn};

use crate::ordering::PoolOrd;
use crate::tx::{PendingTx, PoolTransaction, TxId};
use crate::validation::{ValidationOutcome, Validator};
use crate::TransactionPool;

pub struct Pool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    inner: Arc<Inner<T, V, O>>,
}

struct Inner<T, V, O: PoolOrd> {
    /// List of all valid txs mapped by their hash.
    valid_ids_by_hash: RwLock<HashMap<TxHash, TxId>>,

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

impl<T, V, O> Pool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    /// Creates a new [Pool] with the given [Validator] and [PoolOrd] mechanism.
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

    /// Notifies all listeners about the new incoming transaction.
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
                            hash = format!("{hash:#x}"),
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

impl<T, V, O> TransactionPool for Pool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    type Transaction = T;
    type Validator = V;
    type Ordering = O;

    fn add_transaction(&self, tx: T) {
        let id = TxId::new(tx.sender(), tx.nonce());

        match self.inner.validator.validate(tx) {
            Ok(outcome) => {
                let hash = match outcome {
                    ValidationOutcome::Valid(tx) => {
                        // get the priority of the validated tx
                        let priority = self.inner.ordering.priority(&tx);

                        let pool_tx = PendingTx::new(id.clone(), tx, priority);
                        let hash = pool_tx.tx.hash();

                        // insert the tx in the pool
                        self.inner.valid_ids_by_hash.write().insert(hash, id.clone());
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

                info!(hash = format!("{hash:#x}"), "Transaction received.");
            }

            Err(error @ crate::validation::Error { hash, .. }) => {
                error!(hash = format!("{hash:#x}"), %error, "Failed to validate transaction.");
            }
        }
    }

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
    fn remove_transactions(&self, hashes: &[TxHash]) {
        let ids = hashes
            .iter()
            .filter_map(|hash| self.inner.valid_ids_by_hash.read().get(hash).cloned())
            .collect::<Vec<TxId>>();

        // get the locks on all the pools first
        let mut all = self.inner.valid_txs.write();
        let mut pending = self.inner.pending_txs.write();

        for id in ids {
            all.remove(&id);
            pending.retain(|tx| tx.id != id);
        }
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

impl<T, V, O> Clone for Pool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
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
pub(crate) mod test_utils {

    use katana_primitives::contract::{ContractAddress, Nonce};
    use katana_primitives::FieldElement;
    use rand::Rng;

    use super::*;
    use crate::ordering::Fcfs;
    use crate::tx::PoolTransaction;
    use crate::validation::NoopValidator;

    fn random_bytes<const SIZE: usize>() -> [u8; SIZE] {
        let mut bytes = [0u8; SIZE];
        rand::thread_rng().fill(&mut bytes[..]);
        bytes
    }

    #[derive(Clone, Debug)]
    pub struct PoolTx {
        tip: u64,
        nonce: Nonce,
        hash: TxHash,
        max_fee: u128,
        sender: ContractAddress,
    }

    impl PoolTx {
        pub fn new() -> Self {
            Self {
                tip: rand::thread_rng().gen(),
                max_fee: rand::thread_rng().gen(),
                hash: TxHash::from_bytes_be(&random_bytes::<32>()),
                nonce: Nonce::from_bytes_be(&random_bytes::<32>()),
                sender: {
                    let felt = FieldElement::from_bytes_be(&random_bytes::<32>());
                    ContractAddress::from(felt)
                },
            }
        }

        pub fn with_tip(mut self, tip: u64) -> Self {
            self.tip = tip;
            self
        }
    }

    impl PoolTransaction for PoolTx {
        fn hash(&self) -> TxHash {
            self.hash
        }

        fn max_fee(&self) -> u128 {
            self.max_fee
        }

        fn nonce(&self) -> Nonce {
            self.nonce
        }

        fn sender(&self) -> ContractAddress {
            self.sender
        }

        fn tip(&self) -> u64 {
            self.tip
        }
    }

    pub type MockTxPool<V, O> = Pool<PoolTx, V, O>;

    /// Create a tx pool that uses a noop validator and a first-come-first-serve ordering.
    pub fn mock_pool() -> MockTxPool<NoopValidator<PoolTx>, Fcfs<PoolTx>> {
        MockTxPool::new(NoopValidator::new(), Fcfs::new())
    }
}

#[cfg(test)]
mod tests {

    use super::test_utils::*;
    use crate::tx::PoolTransaction;
    use crate::TransactionPool;

    #[test]
    fn pool_operations() {
        let txs = [
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
        ];

        let pool = mock_pool();

        // initially pool should be empty
        assert!(pool.size() == 0);
        assert!(pool.inner.valid_txs.read().is_empty());
        assert!(pool.inner.pending_txs.read().is_empty());
        assert!(pool.inner.rejected_txs.read().is_empty());

        // add all the txs to the pool
        txs.iter().for_each(|tx| pool.add_transaction(tx.clone()));

        // all the txs should be in the pool
        assert_eq!(pool.size(), txs.len());
        assert_eq!(pool.inner.valid_txs.read().len(), txs.len());
        assert_eq!(pool.inner.valid_txs.read().len(), pool.inner.pending_txs.read().len());
        assert!(txs.iter().all(|tx| pool.get(tx.hash()).is_some()));

        // noop validator should consider all txs as valid
        let pendings = pool.pending_transactions().collect::<Vec<_>>();
        assert_eq!(pendings.len(), txs.len());

        // bcs we're using fcfs, the order should be the same as the order of the txs submission
        for (pending, original) in pendings.iter().zip(txs.iter()) {
            assert_eq!(pending.tx.tip(), original.tip());
            assert_eq!(pending.tx.hash(), original.hash());
            assert_eq!(pending.tx.nonce(), original.nonce());
            assert_eq!(pending.tx.sender(), original.sender());
            assert_eq!(pending.tx.max_fee(), original.max_fee());
        }

        // the txs list in valid_txs must be a superset of the pending_txs
        pool.inner.valid_txs.read().iter().for_each(|(k, _)| {
            assert!(pool.inner.valid_txs.read().contains_key(k));
        });

        // remove all the transactions from the pool
        let hashes = txs.iter().map(|tx| tx.hash()).collect::<Vec<_>>();
        pool.remove_transactions(&hashes);

        // all txs should've been removed
        assert!(pool.size() == 0);
        assert!(pool.inner.valid_txs.read().is_empty());
        assert!(pool.inner.pending_txs.read().is_empty());
        assert!(pool.inner.rejected_txs.read().is_empty());
        assert!(txs.iter().all(|tx| pool.get(tx.hash()).is_none()));
    }

    #[tokio::test]
    async fn tx_listeners() {
        let txs = [
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
        ];

        let pool = mock_pool();
        // register a listener for incoming txs
        let mut listener = pool.add_listener();

        // start adding txs to the pool
        txs.iter().for_each(|tx| pool.add_transaction(tx.clone()));

        // the channel should contain all the added txs
        let mut counter = 0;
        while let Ok(Some(hash)) = listener.try_next() {
            counter += 1;
            assert!(txs.iter().find(|tx| tx.hash() == hash).is_some());
        }

        // we should be notified exactly the same number of txs as we added
        assert_eq!(counter, txs.len());
    }
}
