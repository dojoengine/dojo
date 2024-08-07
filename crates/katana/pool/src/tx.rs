use std::sync::Arc;

use katana_executor::ExecutionError;
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::transaction::TxHash;

use crate::ordering::PoolOrd;

// the transaction type is recommended to implement a cheap clone (eg ref-counting) so that it
// can be cloned around to different pools as necessary.
pub trait PoolTransaction: Ord + Clone {
    /// return the id of this pool txn.
    fn id(&self) -> &TxId;
    /// return the tx hash.
    fn hash(&self) -> &TxHash;
    /// return the tx nonce.
    fn nonce(&self) -> &Nonce;
    /// return the tx sender.
    fn sender(&self) -> &ContractAddress;
}

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
pub struct ValidTx<T, O: PoolOrd> {
    pub tx: Arc<T>,
    pub priority: O::PriorityValue,
}

impl<T, O: PoolOrd> Clone for ValidTx<T, O> {
    fn clone(&self) -> Self {
        Self { tx: Arc::clone(&self.tx), priority: self.priority.clone() }
    }
}

#[derive(Debug)]
pub struct PendingTx<T, O: PoolOrd> {
    pub id: TxId,
    pub tx: ValidTx<T, O>,
}

impl<T, O: PoolOrd> PendingTx<T, O> {
    pub fn new(id: TxId, tx: ValidTx<T, O>) -> Self {
        Self { id, tx }
    }
}

impl<T, O> Clone for PendingTx<T, O>
where
    O: PoolOrd,
{
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), tx: self.tx.clone() }
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

pub struct InvalidTx<T> {
    pub tx: Arc<T>,
    pub error: ExecutionError,
}

impl<T> InvalidTx<T> {
    pub fn new(tx: T, error: ExecutionError) -> Self {
        Self { tx: Arc::new(tx), error }
    }
}
