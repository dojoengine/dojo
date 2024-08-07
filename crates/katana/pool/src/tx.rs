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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TxId {
    sender: ContractAddress,
    nonce: Nonce,
}

impl TxId {
    pub fn descendent(&self) -> Self {
        Self { sender: self.sender, nonce: self.nonce + 1 }
    }
}

pub struct PoolTx<T, O: PoolOrd> {
    pub id: TxId,
    pub tx: T,
    pub priority: O::PriorityValue,
}

impl<T, O: PoolOrd> PartialEq for PoolTx<T, O> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl<T, O: PoolOrd> Eq for PoolTx<T, O> {}

impl<T, O: PoolOrd> PartialOrd for PoolTx<T, O> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T, O: PoolOrd> Ord for PoolTx<T, O> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}
