use std::sync::Arc;
use std::vec::IntoIter;

use futures::channel::mpsc::{channel, Receiver, Sender};
use katana_primitives::transaction::TxHash;
use parking_lot::RwLock;
use tracing::{error, info, warn};

use crate::ordering::PoolOrd;
use crate::tx::{PendingTx, PoolTransaction, TxId};
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
    /// List of all valid txs in the pool
    transactions: RwLock<Vec<PendingTx<T, O>>>,

    /// listeners for incoming txs
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
                transactions: Default::default(),
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
                        self.inner.transactions.write().push(tx);
                        self.notify_listener(hash);

                        Ok(hash)
                    }

                    ValidationOutcome::Invalid { tx, error } => {
                        warn!(hash = format!("{:#x}", tx.hash()), "Invalid transaction.");
                        Err(error.into())
                    }
                }
            }

            Err(e @ crate::validation::Error { hash, .. }) => {
                error!(hash = format!("{hash:#x}"), %e, "Failed to validate transaction.");
                Err(PoolError::Internal(e.error))
            }
        }
    }

    fn take_transactions(&self) -> impl Iterator<Item = PendingTx<T, O>> {
        // take all the transactions
        PendingTransactions {
            all: std::mem::take(&mut *self.inner.transactions.write()).into_iter(),
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

/// an iterator that yields transactions from the pool that can be included in a block, sorted by
/// by its priority.
struct PendingTransactions<T, O: PoolOrd> {
    all: IntoIter<PendingTx<T, O>>,
}

impl<T, O> Iterator for PendingTransactions<T, O>
where
    T: PoolTransaction,
    O: PoolOrd<Transaction = T>,
{
    type Item = PendingTx<T, O>;

    fn next(&mut self) -> Option<Self::Item> {
        self.all.next()
    }
}

#[cfg(test)]
pub(crate) mod test_utils {

    use katana_executor::ExecutionError;
    use katana_primitives::contract::{ContractAddress, Nonce};
    use katana_primitives::FieldElement;
    use rand::Rng;

    use super::*;
    use crate::tx::PoolTransaction;
    use crate::validation::{
        InvalidTransactionError, ValidationOutcome, ValidationResult, Validator,
    };

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
        #[allow(clippy::new_without_default)]
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

    /// A tip-based validator that flags transactions as invalid if they have less than 10 tip.
    pub struct TipValidator<T> {
        threshold: u64,
        t: std::marker::PhantomData<T>,
    }

    impl<T> TipValidator<T> {
        pub fn new(threshold: u64) -> Self {
            Self { threshold, t: std::marker::PhantomData }
        }
    }

    impl<T: PoolTransaction> Validator for TipValidator<T> {
        type Transaction = T;

        fn validate(&self, tx: Self::Transaction) -> ValidationResult<Self::Transaction> {
            if tx.tip() < self.threshold {
                return ValidationResult::Ok(ValidationOutcome::Invalid {
                    tx,
                    error: InvalidTransactionError::InsufficientFunds {},
                });
            }

            ValidationResult::Ok(ValidationOutcome::Valid(tx))
        }
    }
}

#[cfg(test)]
mod tests {

    use katana_primitives::contract::{ContractAddress, Nonce};
    use katana_primitives::FieldElement;

    use super::test_utils::*;
    use super::Pool;
    use crate::ordering::{self, FiFo};
    use crate::pool::test_utils;
    use crate::tx::PoolTransaction;
    use crate::validation::{NoopValidator, ValidationOutcome, Validator};
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
        let pendings = pool.take_transactions().collect::<Vec<_>>();
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
        let _ = pool.take_transactions();

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
    #[ignore = "Rejected pool not implemented yet"]
    fn transactions_rejected() {
        let all = [
            PoolTx::new().with_tip(5),
            PoolTx::new().with_tip(0),
            PoolTx::new().with_tip(15),
            PoolTx::new().with_tip(8),
            PoolTx::new().with_tip(12),
            PoolTx::new().with_tip(10),
            PoolTx::new().with_tip(1),
        ];

        // create a pool with a validator that rejects txs with tip < 10
        let pool = Pool::new(test_utils::TipValidator::new(10), FiFo::new());

        // Extract the expected valid and invalid transactions from the all list
        let (expected_valids, expected_invalids) = pool
            .validator()
            .validate_all(all.to_vec())
            .into_iter()
            .filter_map(|res| res.ok())
            .fold((Vec::new(), Vec::new()), |mut acc, res| match res {
                ValidationOutcome::Valid(tx) => {
                    acc.0.push(tx);
                    acc
                }

                ValidationOutcome::Invalid { tx, .. } => {
                    acc.1.push(tx);
                    acc
                }
            });

        assert_eq!(expected_valids.len(), 3);
        assert_eq!(expected_invalids.len(), 4);

        // Add all transactions to the pool
        all.iter().for_each(|tx| {
            let _ = pool.add_transaction(tx.clone());
        });

        // Check that all transactions should be in the pool regardless of validity
        assert!(all.iter().all(|tx| pool.get(tx.hash()).is_some()));
        assert_eq!(pool.size(), all.len());

        // Pending transactions should only contain the valid transactions
        let pendings = pool.take_transactions().collect::<Vec<_>>();
        assert_eq!(pendings.len(), expected_valids.len());

        // bcs its a fcfs pool, the order of the pending txs should be the as its order of insertion
        // (position in the array)
        for (actual, expected) in pendings.iter().zip(expected_valids.iter()) {
            assert_eq!(actual.tx.hash(), expected.hash());
        }

        // // rejected_txs should contain all the invalid txs
        // assert_eq!(pool.inner.rejected.read().len(), expected_invalids.len());
        // for tx in expected_invalids.iter() {
        //     assert!(pool.inner.rejected.read().contains_key(&tx.hash()));
        // }
    }

    #[test]
    #[ignore = "Txs ordering not fully implemented yet"]
    fn txs_ordering() {
        // Create mock transactions with different tips and in random order
        let txs = [
            PoolTx::new().with_tip(1),
            PoolTx::new().with_tip(6),
            PoolTx::new().with_tip(3),
            PoolTx::new().with_tip(2),
            PoolTx::new().with_tip(5),
            PoolTx::new().with_tip(4),
            PoolTx::new().with_tip(7),
        ];

        // Create a pool with tip-based ordering
        let pool = Pool::new(NoopValidator::new(), ordering::Tip::new());

        // Add transactions to the pool
        txs.iter().for_each(|tx| {
            let _ = pool.add_transaction(tx.clone());
        });

        // Get pending transactions
        let pending = pool.take_transactions().collect::<Vec<_>>();

        // Assert that the transactions are ordered by tip (highest to lowest)
        assert_eq!(pending[0].tx.tip(), 7);
        assert_eq!(pending[1].tx.tip(), 6);
        assert_eq!(pending[2].tx.tip(), 5);
        assert_eq!(pending[3].tx.tip(), 4);
        assert_eq!(pending[4].tx.tip(), 3);
        assert_eq!(pending[5].tx.tip(), 2);
        assert_eq!(pending[6].tx.tip(), 1);
    }

    #[test]
    #[ignore = "Txs dependency management not fully implemented yet"]
    fn dependent_txs_linear_insertion() {
        let pool = TestPool::test();

        // Create 100 transactions with the same sender but increasing nonce
        let total = 100u128;
        let sender = ContractAddress::from(FieldElement::from_hex("0x1337").unwrap());
        let txs: Vec<PoolTx> = (0..total)
            .map(|i| PoolTx::new().with_sender(sender).with_nonce(Nonce::from(i)))
            .collect();

        // Add all transactions to the pool
        txs.iter().for_each(|tx| {
            let _ = pool.add_transaction(tx.clone());
        });

        // Get pending transactions
        let pending = pool.take_transactions().collect::<Vec<_>>();

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
