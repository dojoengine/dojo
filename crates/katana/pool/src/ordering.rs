use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::PoolTransaction;

// evaluates the priority of a transaction which would be used to determine how txs are ordered in
// the pool.
pub trait PoolOrd {
    type Transaction: PoolTransaction;
    /// The priority value type whose [Ord] implementation is used to order the transaction in the
    /// pool.
    type PriorityValue: Ord + Clone + fmt::Debug;

    /// returns the priority value for the given transaction
    fn priority(&self, tx: &Self::Transaction) -> Self::PriorityValue;
}

/// First-come-first-serve ordering implementation.
///
/// This ordering implementation can be generic over any transaction type as it doesn't require any
/// context on the tx data itself.
#[derive(Debug)]
pub struct FiFo<T> {
    nonce: AtomicU64,
    _tx: PhantomData<T>,
}

impl<T> FiFo<T> {
    pub fn new() -> Self {
        Self { nonce: AtomicU64::new(0), _tx: PhantomData }
    }
}

impl<T: PoolTransaction> PoolOrd for FiFo<T> {
    type Transaction = T;
    type PriorityValue = TxSubmissionNonce;

    fn priority(&self, _: &Self::Transaction) -> Self::PriorityValue {
        TxSubmissionNonce(self.nonce.fetch_add(1, AtomicOrdering::Relaxed))
    }
}

impl<T> Default for FiFo<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TxSubmissionNonce(u64);

impl Ord for TxSubmissionNonce {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse the ordering so lower values have higher priority
        self.0.cmp(&other.0)
    }
}

impl Eq for TxSubmissionNonce {}

impl PartialOrd for TxSubmissionNonce {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for TxSubmissionNonce {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

/// Tip-based ordering implementation.
///
/// This ordering implementation uses the transaction's tip as the priority value. We don't have a
/// use case for this ordering implementation yet, but it's mostly used for testing.
#[derive(Debug)]
pub struct TipOrdering<T>(PhantomData<T>);

impl<T> TipOrdering<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

#[derive(Debug, Clone)]
pub struct Tip(u64);

impl Ord for Tip {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.cmp(&self.0)
    }
}

impl PartialOrd for Tip {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Tip {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Tip {}

impl<T: PoolTransaction> PoolOrd for TipOrdering<T> {
    type Transaction = T;
    type PriorityValue = Tip;

    fn priority(&self, tx: &Self::Transaction) -> Self::PriorityValue {
        Tip(tx.tip())
    }
}

impl<T> Default for TipOrdering<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {

    use futures::executor;

    use crate::ordering::{self, FiFo};
    use crate::pool::test_utils::*;
    use crate::tx::PoolTransaction;
    use crate::validation::NoopValidator;
    use crate::{Pool, TransactionPool};

    #[test]
    fn fifo_ordering() {
        // Create mock transactions
        let txs = [PoolTx::new(), PoolTx::new(), PoolTx::new(), PoolTx::new(), PoolTx::new()];

        // Create a pool with FiFo ordering
        let pool = Pool::new(NoopValidator::new(), FiFo::new());

        // Add transactions to the pool
        txs.iter().for_each(|tx| {
            let _ = pool.add_transaction(tx.clone());
        });

        // Get pending transactions
        let pendings = executor::block_on_stream(pool.pending_transactions());

        // Assert that the transactions are in the order they were added (first to last)
        pendings.into_iter().zip(txs).for_each(|(pending, tx)| {
            assert_eq!(pending.tx.as_ref(), &tx);
        });
    }

    #[test]
    fn tip_based_ordering() {
        // Create mock transactions with different tips and in random order
        let txs = [
            PoolTx::new().with_tip(2),
            PoolTx::new().with_tip(1),
            PoolTx::new().with_tip(6),
            PoolTx::new().with_tip(3),
            PoolTx::new().with_tip(2),
            PoolTx::new().with_tip(2),
            PoolTx::new().with_tip(5),
            PoolTx::new().with_tip(4),
            PoolTx::new().with_tip(7),
        ];

        // Create a pool with tip-based ordering
        let pool = Pool::new(NoopValidator::new(), ordering::TipOrdering::new());

        // Add transactions to the pool
        txs.iter().for_each(|tx| {
            let _ = pool.add_transaction(tx.clone());
        });

        // Get pending transactions
        let pending = executor::block_on_stream(pool.pending_transactions()).collect::<Vec<_>>();
        assert_eq!(pending.len(), txs.len());

        // Assert that the transactions are ordered by tip (highest to lowest)
        assert_eq!(pending[0].tx.tip(), 7);
        assert_eq!(pending[0].tx.hash(), txs[8].hash());

        assert_eq!(pending[1].tx.tip(), 6);
        assert_eq!(pending[1].tx.hash(), txs[2].hash());

        assert_eq!(pending[2].tx.tip(), 5);
        assert_eq!(pending[2].tx.hash(), txs[6].hash());

        assert_eq!(pending[3].tx.tip(), 4);
        assert_eq!(pending[3].tx.hash(), txs[7].hash());

        assert_eq!(pending[4].tx.tip(), 3);
        assert_eq!(pending[4].tx.hash(), txs[3].hash());

        assert_eq!(pending[5].tx.tip(), 2);
        assert_eq!(pending[5].tx.hash(), txs[0].hash());

        assert_eq!(pending[6].tx.tip(), 2);
        assert_eq!(pending[6].tx.hash(), txs[4].hash());

        assert_eq!(pending[7].tx.tip(), 2);
        assert_eq!(pending[7].tx.hash(), txs[5].hash());

        assert_eq!(pending[8].tx.tip(), 1);
        assert_eq!(pending[8].tx.hash(), txs[1].hash());
    }
}
