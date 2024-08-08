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
pub struct Fcfs<T> {
    nonce: AtomicU64,
    _tx: PhantomData<T>,
}

impl<T: PoolTransaction> Fcfs<T> {
    pub fn new() -> Self {
        Self { nonce: AtomicU64::new(0), _tx: PhantomData }
    }
}

impl<T: PoolTransaction> PoolOrd for Fcfs<T> {
    type Transaction = T;
    type PriorityValue = TxSubmissionNonce;

    fn priority(&self, _: &Self::Transaction) -> Self::PriorityValue {
        TxSubmissionNonce(self.nonce.fetch_add(1, AtomicOrdering::Relaxed))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TxSubmissionNonce(u64);

impl Ord for TxSubmissionNonce {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse the ordering so lower values have higher priority
        other.0.cmp(&self.0)
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
pub struct Tip<T>(PhantomData<T>);

impl<T: PoolTransaction> Tip<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: PoolTransaction> PoolOrd for Tip<T> {
    type Transaction = T;
    type PriorityValue = u64;

    fn priority(&self, tx: &Self::Transaction) -> Self::PriorityValue {
        tx.tip()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::test_utils::PoolTx;
    use crate::pool::Pool;
    use crate::validation::NoopValidator;
    use crate::TransactionPool;

    #[test]
    fn tip_ordering() {
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
        let pool = Pool::new(NoopValidator::new(), Tip::new());

        // Add transactions to the pool
        txs.iter().for_each(|tx| pool.add_transaction(tx.clone()));

        // Get pending transactions
        let pending = pool.pending_transactions().collect::<Vec<_>>();

        // Assert that the transactions are ordered by tip (highest to lowest)
        assert_eq!(pending[0].tx.tip(), 7);
        assert_eq!(pending[1].tx.tip(), 6);
        assert_eq!(pending[2].tx.tip(), 5);
        assert_eq!(pending[3].tx.tip(), 4);
        assert_eq!(pending[4].tx.tip(), 3);
        assert_eq!(pending[5].tx.tip(), 2);
        assert_eq!(pending[6].tx.tip(), 1);
    }
}
