use core::fmt;
use std::collections::BTreeSet;
use std::sync::Arc;

use futures::channel::mpsc::{channel, Receiver, Sender};
use katana_primitives::transaction::TxHash;
use parking_lot::RwLock;
use tokio::sync::Notify;
use tracing::{error, info, warn};

use crate::ordering::PoolOrd;
use crate::pending::PendingTransactions;
use crate::subscription::PoolSubscription;
use crate::tx::{PendingTx, PoolTransaction, TxId};
use crate::validation::error::InvalidTransactionError;
use crate::validation::{ValidationOutcome, Validator};
use crate::{PoolError, PoolResult, TransactionPool};

#[derive(Debug)]
pub struct Pool<T, V, O>
where
    T: PoolTransaction,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    inner: Arc<Inner<T, V, O>>,
}

#[derive(Debug)]
struct Inner<T, V, O: PoolOrd> {
    /// List of all valid txs in the pool.
    transactions: RwLock<BTreeSet<PendingTx<T, O>>>,

    /// listeners for incoming txs
    listeners: RwLock<Vec<Sender<TxHash>>>,

    /// subscribers for incoming txs
    subscribers: RwLock<Vec<PoolSubscription<T, O>>>,

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
                transactions: Default::default(),
                subscribers: Default::default(),
                listeners: Default::default(),
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

    // notify both listener and subscribers
    fn notify(&self, tx: PendingTx<T, O>) {
        self.notify_listener(tx.tx.hash());
        self.notify_subscribers(tx);
    }

    fn notify_subscribers(&self, tx: PendingTx<T, O>) {
        let subscribers = self.inner.subscribers.read();
        for subscriber in subscribers.iter() {
            subscriber.broadcast(tx.clone());
        }
    }

    fn subscribe(&self) -> PoolSubscription<T, O> {
        let notify = Arc::new(Notify::new());
        let subscription = PoolSubscription { notify, txs: Default::default() };
        self.inner.subscribers.write().push(subscription.clone());
        subscription
    }
}

impl<T, V, O> TransactionPool for Pool<T, V, O>
where
    T: PoolTransaction + fmt::Debug,
    V: Validator<Transaction = T>,
    O: PoolOrd<Transaction = T>,
{
    type Transaction = T;
    type Validator = V;
    type Ordering = O;

    fn add_transaction(&self, tx: T) -> PoolResult<TxHash> {
        let hash = tx.hash();
        let id = TxId::new(tx.sender(), tx.nonce());

        info!(hash = format!("{hash:#x}"), "Transaction received.");

        match self.inner.validator.validate(tx) {
            Ok(outcome) => {
                match outcome {
                    ValidationOutcome::Valid(tx) => {
                        // get the priority of the validated tx
                        let priority = self.inner.ordering.priority(&tx);
                        let tx = PendingTx::new(id, tx, priority);

                        // insert the tx in the pool
                        self.inner.transactions.write().insert(tx.clone());
                        self.notify(tx);

                        Ok(hash)
                    }

                    // TODO: create a small cache for rejected transactions to respect the rpc spec
                    // `getTransactionStatus`
                    ValidationOutcome::Invalid { error, .. } => {
                        warn!(hash = format!("{hash:#x}"), "Invalid transaction.");
                        Err(PoolError::InvalidTransaction(Box::new(error)))
                    }

                    // return as error for now but ideally we should kept the tx in a separate
                    // queue and revalidate it when the parent tx is added to the pool
                    ValidationOutcome::Dependent { tx, tx_nonce, current_nonce } => {
                        info!(hash = format!("{hash:#x}"), "Dependent transaction.");
                        let err = InvalidTransactionError::InvalidNonce {
                            address: tx.sender(),
                            current_nonce,
                            tx_nonce,
                        };
                        Err(PoolError::InvalidTransaction(Box::new(err)))
                    }
                }
            }

            Err(e @ crate::validation::Error { hash, .. }) => {
                error!(hash = format!("{hash:#x}"), %e, "Failed to validate transaction.");
                Err(PoolError::Internal(e.error))
            }
        }
    }

    fn pending_transactions(&self) -> PendingTransactions<Self::Transaction, Self::Ordering> {
        // take all the transactions
        PendingTransactions {
            subscription: self.subscribe(),
            all: self.inner.transactions.read().clone().into_iter(),
        }
    }

    // check if a tx is in the pool
    fn contains(&self, hash: TxHash) -> bool {
        self.get(hash).is_some()
    }

    fn get(&self, hash: TxHash) -> Option<Arc<T>> {
        self.inner
            .transactions
            .read()
            .iter()
            .find(|tx| tx.tx.hash() == hash)
            .map(|t| Arc::clone(&t.tx))
    }

    fn add_listener(&self) -> Receiver<TxHash> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.inner.listeners.write().push(tx);
        rx
    }

    fn remove_transactions(&self, hashes: &[TxHash]) {
        // retain only transactions that aren't included in the list
        let mut txs = self.inner.transactions.write();
        txs.retain(|t| !hashes.contains(&t.tx.hash()))
    }

    fn size(&self) -> usize {
        self.inner.transactions.read().len()
    }

    fn validator(&self) -> &Self::Validator {
        &self.inner.validator
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

#[cfg(test)]
pub(crate) mod test_utils {

    use katana_primitives::contract::{ContractAddress, Nonce};
    use katana_primitives::Felt;
    use rand::Rng;

    use super::*;
    use crate::tx::PoolTransaction;

    fn random_bytes<const SIZE: usize>() -> [u8; SIZE] {
        let mut bytes = [0u8; SIZE];
        rand::thread_rng().fill(&mut bytes[..]);
        bytes
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct PoolTx {
        tip: u64,
        nonce: Nonce,
        hash: TxHash,
        max_fee: u128,
        sender: ContractAddress,
    }

    impl PoolTx {
        #[allow(clippy::new_without_default)]
        pub fn new() -> Self {
            Self {
                tip: rand::thread_rng().gen(),
                max_fee: rand::thread_rng().gen(),
                hash: TxHash::from_bytes_be(&random_bytes::<32>()),
                nonce: Nonce::from_bytes_be(&random_bytes::<32>()),
                sender: {
                    let felt = Felt::from_bytes_be(&random_bytes::<32>());
                    ContractAddress::from(felt)
                },
            }
        }

        pub fn with_tip(mut self, tip: u64) -> Self {
            self.tip = tip;
            self
        }

        pub fn with_sender(mut self, sender: ContractAddress) -> Self {
            self.sender = sender;
            self
        }

        pub fn with_nonce(mut self, nonce: Nonce) -> Self {
            self.nonce = nonce;
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
}

#[cfg(test)]
mod tests {

    use katana_primitives::contract::{ContractAddress, Nonce};
    use katana_primitives::Felt;

    use super::test_utils::*;
    use super::Pool;
    use crate::ordering::FiFo;
    use crate::tx::PoolTransaction;
    use crate::validation::NoopValidator;
    use crate::TransactionPool;

    /// Tx pool that uses a noop validator and a first-come-first-serve ordering.
    type TestPool = Pool<PoolTx, NoopValidator<PoolTx>, FiFo<PoolTx>>;

    impl TestPool {
        fn test() -> Self {
            Pool::new(NoopValidator::new(), FiFo::new())
        }
    }

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

        let pool = TestPool::test();

        // initially pool should be empty
        assert!(pool.size() == 0);
        assert!(pool.inner.transactions.read().is_empty());

        // add all the txs to the pool
        txs.iter().for_each(|tx| {
            let _ = pool.add_transaction(tx.clone());
        });

        // all the txs should be in the pool
        assert_eq!(pool.size(), txs.len());
        assert_eq!(pool.inner.transactions.read().len(), txs.len());
        assert!(txs.iter().all(|tx| pool.get(tx.hash()).is_some()));

        // noop validator should consider all txs as valid
        let pendings = pool.pending_transactions().collect::<Vec<_>>();
        assert_eq!(pendings.len(), txs.len());

        // bcs we're using fcfs, the order should be the same as the order of the txs submission
        // (position in the array)
        for (actual, expected) in pendings.iter().zip(txs.iter()) {
            assert_eq!(actual.tx.tip(), expected.tip());
            assert_eq!(actual.tx.hash(), expected.hash());
            assert_eq!(actual.tx.nonce(), expected.nonce());
            assert_eq!(actual.tx.sender(), expected.sender());
            assert_eq!(actual.tx.max_fee(), expected.max_fee());
        }

        // take all transactions
        let _ = pool.pending_transactions();

        // all txs should've been removed
        assert!(pool.size() == 0);
        assert!(pool.inner.transactions.read().is_empty());
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

        let pool = TestPool::test();
        // register a listener for incoming txs
        let mut listener = pool.add_listener();

        // start adding txs to the pool
        txs.iter().for_each(|tx| {
            let _ = pool.add_transaction(tx.clone());
        });

        // the channel should contain all the added txs
        let mut counter = 0;
        while let Ok(Some(hash)) = listener.try_next() {
            counter += 1;
            assert!(txs.iter().any(|tx| tx.hash() == hash));
        }

        // we should be notified exactly the same number of txs as we added
        assert_eq!(counter, txs.len());
    }

    #[test]
    #[ignore = "Txs dependency management not fully implemented yet"]
    fn dependent_txs_linear_insertion() {
        let pool = TestPool::test();

        // Create 100 transactions with the same sender but increasing nonce
        let total = 100u128;
        let sender = ContractAddress::from(Felt::from_hex("0x1337").unwrap());
        let txs: Vec<PoolTx> = (0..total)
            .map(|i| PoolTx::new().with_sender(sender).with_nonce(Nonce::from(i)))
            .collect();

        // Add all transactions to the pool
        txs.iter().for_each(|tx| {
            let _ = pool.add_transaction(tx.clone());
        });

        // Get pending transactions
        let pending = pool.pending_transactions().collect::<Vec<_>>();

        // Check that the number of pending transactions matches the number of added transactions
        assert_eq!(pending.len(), total as usize);

        // Check that the pending transactions are in the same order as they were added
        for (i, pending_tx) in pending.iter().enumerate() {
            assert_eq!(pending_tx.tx.nonce(), Nonce::from(i as u128));
            assert_eq!(pending_tx.tx.sender(), sender);
        }
    }

    #[test]
    #[ignore = "Txs dependency management not fully implemented yet"]
    fn dependent_txs_random_insertion() {}
}
