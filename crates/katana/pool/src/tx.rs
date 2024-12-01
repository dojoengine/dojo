use std::sync::Arc;
use std::time::Instant;

use katana_primitives::contract::{ContractAddress, Nonce};
use katana_primitives::transaction::{
    DeclareTx, DeployAccountTx, ExecutableTx, ExecutableTxWithHash, InvokeTx, TxHash,
};

use crate::ordering::PoolOrd;

// the transaction type is recommended to implement a cheap clone (eg ref-counting) so that it
// can be cloned around to different pools as necessary.
pub trait PoolTransaction: Clone {
    /// return the tx hash.
    fn hash(&self) -> TxHash;

    /// return the tx nonce.
    fn nonce(&self) -> Nonce;

    /// return the tx sender.
    fn sender(&self) -> ContractAddress;

    /// return the max fee that tx is willing to pay.
    fn max_fee(&self) -> u128;

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
    pub added_at: std::time::Instant,
}

impl<T, O: PoolOrd> PendingTx<T, O> {
    pub fn new(id: TxId, tx: T, priority: O::PriorityValue) -> Self {
        Self { id, tx: Arc::new(tx), priority, added_at: Instant::now() }
    }
}

// We can't just derive these traits because the derive implementation would require that
// the generics also implement these traits, which is not necessary.

impl<T, O: PoolOrd> Clone for PendingTx<T, O> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            added_at: self.added_at,
            tx: Arc::clone(&self.tx),
            priority: self.priority.clone(),
        }
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

// When two transactions have the same priority, we want to prioritize the one that was added
// first. So, when an incoming transaction with similar priority value is added to the
// [BTreeSet](std::collections::BTreeSet), the transaction is assigned a 'greater'
// [Ordering](std::cmp::Ordering) so that it will be placed after the existing ones. This is
// because items in a BTree is ordered from lowest to highest.
impl<T, O: PoolOrd> Ord for PendingTx<T, O> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.priority.cmp(&other.priority) {
            std::cmp::Ordering::Equal => std::cmp::Ordering::Greater,
            other => other,
        }
    }
}

impl PoolTransaction for ExecutableTxWithHash {
    fn hash(&self) -> TxHash {
        self.hash
    }

    fn nonce(&self) -> Nonce {
        match &self.transaction {
            ExecutableTx::Invoke(tx) => match tx {
                InvokeTx::V0(v0) => v0.nonce,
                InvokeTx::V1(v1) => v1.nonce,
                InvokeTx::V3(v3) => v3.nonce,
            },
            ExecutableTx::L1Handler(tx) => tx.nonce,
            ExecutableTx::Declare(tx) => match &tx.transaction {
                DeclareTx::V1(v1) => v1.nonce,
                DeclareTx::V2(v2) => v2.nonce,
                DeclareTx::V3(v3) => v3.nonce,
            },
            ExecutableTx::DeployAccount(tx) => match tx {
                DeployAccountTx::V1(v1) => v1.nonce,
                DeployAccountTx::V3(v3) => v3.nonce,
            },
        }
    }

    fn sender(&self) -> ContractAddress {
        match &self.transaction {
            ExecutableTx::Invoke(tx) => match tx {
                InvokeTx::V0(v1) => v1.sender_address,
                InvokeTx::V1(v1) => v1.sender_address,
                InvokeTx::V3(v3) => v3.sender_address,
            },
            ExecutableTx::L1Handler(tx) => tx.contract_address,
            ExecutableTx::Declare(tx) => match &tx.transaction {
                DeclareTx::V1(v1) => v1.sender_address,
                DeclareTx::V2(v2) => v2.sender_address,
                DeclareTx::V3(v3) => v3.sender_address,
            },
            ExecutableTx::DeployAccount(tx) => tx.contract_address(),
        }
    }

    fn max_fee(&self) -> u128 {
        match &self.transaction {
            ExecutableTx::Invoke(tx) => match tx {
                InvokeTx::V0(..) => 0, // V0 doesn't have max_fee
                InvokeTx::V1(v1) => v1.max_fee,
                InvokeTx::V3(_) => 0, // V3 doesn't have max_fee
            },
            ExecutableTx::L1Handler(tx) => tx.paid_fee_on_l1,
            ExecutableTx::Declare(tx) => match &tx.transaction {
                DeclareTx::V1(v1) => v1.max_fee,
                DeclareTx::V2(v2) => v2.max_fee,
                DeclareTx::V3(_) => 0, // V3 doesn't have max_fee
            },
            ExecutableTx::DeployAccount(tx) => match tx {
                DeployAccountTx::V1(v1) => v1.max_fee,
                DeployAccountTx::V3(_) => 0, // V3 doesn't have max_fee
            },
        }
    }

    fn tip(&self) -> u64 {
        match &self.transaction {
            ExecutableTx::Invoke(tx) => match tx {
                InvokeTx::V0(_) => 0, // V0 doesn't have tip
                InvokeTx::V1(_) => 0, // V1 doesn't have tip
                InvokeTx::V3(v3) => v3.tip,
            },
            ExecutableTx::L1Handler(_) => 0, // L1Handler doesn't have tip
            ExecutableTx::Declare(tx) => match &tx.transaction {
                DeclareTx::V1(_) | DeclareTx::V2(_) => 0, // V1 and V2 don't have tip
                DeclareTx::V3(v3) => v3.tip,
            },
            ExecutableTx::DeployAccount(tx) => match tx {
                DeployAccountTx::V1(_) => 0, // V1 doesn't have tip
                DeployAccountTx::V3(v3) => v3.tip,
            },
        }
    }
}
