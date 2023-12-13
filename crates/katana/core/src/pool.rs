// Code adapted from Foundry's Anvil

use futures::channel::mpsc::{channel, Receiver, Sender};
use katana_primitives::transaction::ExecutableTxWithHash;
use parking_lot::RwLock;
use starknet::core::types::FieldElement;
use tracing::{info, warn};

#[derive(Debug, Default)]
pub struct TransactionPool {
    transactions: RwLock<Vec<ExecutableTxWithHash>>,
    transaction_listeners: RwLock<Vec<Sender<FieldElement>>>,
}

impl TransactionPool {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TransactionPool {
    pub fn add_transaction(&self, transaction: ExecutableTxWithHash) {
        let hash = transaction.hash;
        self.transactions.write().push(transaction);

        info!(target: "txpool", "Transaction received | Hash: {hash:#x}");

        // notify listeners of new tx added to the pool
        self.notify_listener(hash)
    }

    pub fn add_listener(&self) -> Receiver<FieldElement> {
        const TX_LISTENER_BUFFER_SIZE: usize = 2048;
        let (tx, rx) = channel(TX_LISTENER_BUFFER_SIZE);
        self.transaction_listeners.write().push(tx);
        rx
    }

    /// Get all the transaction from the pool and clear it.
    pub fn get_transactions(&self) -> Vec<ExecutableTxWithHash> {
        let mut txs = self.transactions.write();
        let transactions = txs.clone();
        txs.clear();
        transactions
    }

    /// notifies all listeners about the transaction
    fn notify_listener(&self, hash: FieldElement) {
        let mut listener = self.transaction_listeners.write();
        // this is basically a retain but with mut reference
        for n in (0..listener.len()).rev() {
            let mut listener_tx = listener.swap_remove(n);
            let retain = match listener_tx.try_send(hash) {
                Ok(()) => true,
                Err(e) => {
                    if e.is_full() {
                        warn!(
                            target: "txpool",
                            "[{:?}] Failed to send tx notification because channel is full",
                            hash,
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
