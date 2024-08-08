use std::sync::Arc;

use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::transaction::TxHash;

use crate::ordering::PoolOrd;

// the transaction type is recommended to implement a cheap clone (eg ref-counting) so that it
// can be cloned around to different pools as necessary.
pub trait PoolTransaction: Ord + Clone {
    /// return the id of this pool txn.
    fn id(&self) -> &TxId;

    /// return the tx hash.
    fn hash(&self) -> TxHash;

    /// return the tx nonce.
    fn nonce(&self) -> Nonce;

    /// return the tx sender.
    fn sender(&self) -> ContractAddress;

    /// return the max fee that tx is willing to pay.
    fn max_fee(&self) -> u64;

    /// return the tx tip.
    fn tip(&self) -> u64;
}

/// the tx id in the pool. identified by its sender and nonce.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TxId {
    sender: ContractAddress,
    nonce: Nonce,
}

impl TxId {
    pub fn new(sender: ContractAddress, nonce: Nonce) -> Self {
        Self { sender, nonce }
    }

    pub fn parent(&self) -> Option<Self> {
        if self.nonce == Nonce::ZERO {
            None
        } else {
            Some(Self { sender: self.sender, nonce: self.nonce - 1 })
        }
    }

    pub fn descendent(&self) -> Self {
        Self { sender: self.sender, nonce: self.nonce + 1 }
    }
}

#[derive(Debug)]
pub struct PendingTx<T, O: PoolOrd> {
    pub id: TxId,
    pub tx: Arc<T>,
    pub priority: O::PriorityValue,
}

impl<T, O: PoolOrd> PendingTx<T, O> {
    pub fn new(id: TxId, tx: T, priority: O::PriorityValue) -> Self {
        Self { id, tx: Arc::new(tx), priority }
    }
}

impl<T, O: PoolOrd> Clone for PendingTx<T, O> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), tx: Arc::clone(&self.tx), priority: self.priority.clone() }
    }
}

impl<T, O: PoolOrd> PartialEq for PendingTx<T, O> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl<T, O: PoolOrd> Eq for PendingTx<T, O> {}

impl<T, O: PoolOrd> PartialOrd for PendingTx<T, O> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T, O: PoolOrd> Ord for PendingTx<T, O> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}
