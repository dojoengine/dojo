// - txs of the same sender must be ordered by nonce (so needs some form of tx ordering mechanism)
// - gets notification something happen to a transaction (new, removed, executed, etc).
// - being able to send transactions (that are valid) but with incremented nonce. allowing for sending bunch of txs at once without waiting
// for the previous one to be executed first (to prevent nonce collision).
// - subscribe to a particular tx and gets notified when something happened to it (optional).

// - stateful validator running on top of the tx pool that validate incoming tx and puts in the pool if valid. (Adding a pre-validation stage would mean we'd be running the validation stage twice)
// - valid txs must be valid against all the txs in the pool as well, not just the one in the pending block

// ### State Changes
//
// Once a new block is mined, the pool needs to be updated with a changeset in order to:
//
//   - remove mined transactions
//

use std::marker::PhantomData;

use katana_primitives::transaction::{ExecutableTxWithHash, TxHash};

pub trait PoolTransaction {
    fn hash(&self) -> &TxHash;

    fn tx(&self) -> &ExecutableTxWithHash;

    /// returns the hash of the pool tx that this tx depends on.
    fn dependent(&self) -> Option<&TxHash> {
        None
    }
}

pub enum PoolTx {
    Tx,
    Pending { dependent: TxHash },
}

impl PoolTransaction for PoolTx {
    fn hash(&self) -> &TxHash {
        todo!()
    }

    fn tx(&self) -> &ExecutableTxWithHash {
        todo!()
    }

    fn dependent(&self) -> Option<&TxHash> {
        match self {
            PoolTx::Pending { dependent: parent } => Some(parent),
            _ => None,
        }
    }
}

pub trait Ordering<Tx: PoolTransaction> {
    // order the given list of transactions. the ordering must be deterministic.
    //
    // the ordering implementation should be aware of the dependencies between the txs in the pool.
    fn order(&self, txs: &mut Vec<Tx>);
}

pub struct TxPool<T, O> {
    /// List of txs. the list must be ordered by whatever ordering mechanism is used.
    pending_txs: Vec<T>,
    /// the ordering mechanism used to order the txs in the pool
    _ordering: PhantomData<O>,
}

impl<T, O> TxPool<T, O>
where
    O: Ordering<T>,
    T: PoolTransaction,
{
    // add tx to the pool
    pub fn add_transaction(&self) {}

    // returns transactions that are ready and valid to be included in the next block.
    //
    // best transactions must be ordered by the ordering mechanism used.
    pub fn best_transactions(&self) -> impl Iterator<Item = &T> {
        self.pending_txs.iter()
    }

    // check if a tx is in the pool
    pub fn contains(&self, hash: TxHash) -> bool {
        self.find(hash).is_some()
    }

    pub fn find(&self, hash: TxHash) -> Option<&PoolTx> {
        todo!()
    }

    // remove tx from the pool
    //
    // this should also remove the tx from the dependencies map along
    // with all the txs that depend on it.
    //
    // removing an element must preserve the ordering of the other txs in the pool.
    pub fn remove_transaction(&mut self, hash: TxHash) {
        todo!()
    }
}

trait TxValidator: Send + Sync {
    type Tx;

    fn validate(
        &self,
        tx: &Self::Tx,
    ) -> Result<TxValidationOutcome<Self::Tx>, Box<dyn std::error::Error>>;

    fn validate_all(
        &self,
        txs: Vec<Self::Tx>,
    ) -> Vec<Result<TxValidationOutcome<Self::Tx>, Box<dyn std::error::Error>>> {
        txs.into_iter().map(|tx| self.validate(&tx)).collect()
    }
}

enum TxValidationOutcome<T> {
    Valid { tx: T }, // valid and can be picked up by the ordering mechanism
    Invalid { tx: T, error: Box<dyn std::error::Error> }, // aka rejected in starknet terms
}

// impl TxValidator for StatefulValidator {}
