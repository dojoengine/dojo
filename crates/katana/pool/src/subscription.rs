use std::collections::BTreeSet;
use std::sync::Arc;

use parking_lot::{Condvar, Mutex};

use crate::ordering::PoolOrd;
use crate::tx::PendingTx;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Subscription has been closed.")]
    SubscriptionClosed,
}

struct Inner<T, O: PoolOrd> {
    condvar: Condvar,
    transactions: Mutex<BTreeSet<PendingTx<T, O>>>,
}

pub(crate) struct Sender<T, O: PoolOrd> {
    inner: Arc<Inner<T, O>>,
}

impl<T, O: PoolOrd> Sender<T, O> {
    fn send(&self, tx: PendingTx<T, O>) -> Result<(), Error> {
        let subscribers = Arc::strong_count(&self.inner);

        // if there are no subscribers, return an error
        if subscribers == 1 {
            return Err(Error::SubscriptionClosed);
        }

        self.inner.transactions.lock().insert(tx);
        let _ = self.inner.condvar.notify_one();
        Ok(())
    }
}

pub struct Receiver<T, O: PoolOrd> {
    inner: Arc<Inner<T, O>>,
    pendings: Option<BTreeSet<PendingTx<T, O>>>,
}

impl<T, O: PoolOrd> Receiver<T, O> {
    pub fn recv(&mut self) -> Result<PendingTx<T, O>, Error> {
        if let Some(mut pendings) = self.pendings.take() {
            if let Some(tx) = pendings.pop_first() {
                self.pendings = Some(pendings);
                return Ok(tx);
            }
        }

        let mut transactions = self.inner.transactions.lock();
        while transactions.is_empty() {
            self.inner.condvar.wait(&mut transactions);
        }

        // if there are no subscribers, return an error
        let subscribers = Arc::strong_count(&self.inner);
        if subscribers == 1 {
            return Err(Error::SubscriptionClosed);
        }

        Ok(transactions.pop_first().expect("qed; must not be empty"))
    }
}
