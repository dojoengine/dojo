use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::PoolTransaction;

// evaluates the priority of a transaction which would be used to determine how txs are ordered in
// the pool.
pub trait PoolOrd {
    type Transaction;
    /// The priority value type whose [Ord] implementation is used to order the transaction in the
    /// pool.
    type PriorityValue: Ord + Clone;

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

impl TxSubmissionNonce {
    fn new() -> Self {
        TxSubmissionNonce::default()
    }
}

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
