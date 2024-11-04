use std::collections::BTreeSet;
use std::future::Future;
use std::pin::{pin, Pin};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Stream;
use parking_lot::RwLock;
use tokio::sync::Notify;

use crate::ordering::PoolOrd;
use crate::tx::{PendingTx, PoolTransaction};

#[derive(Debug)]
pub struct PoolSubscription<T, O: PoolOrd> {
    pub(crate) txs: Arc<RwLock<BTreeSet<PendingTx<T, O>>>>,
    pub(crate) notify: Arc<Notify>,
}

impl<T, O: PoolOrd> Clone for PoolSubscription<T, O> {
    fn clone(&self) -> Self {
        Self { txs: self.txs.clone(), notify: self.notify.clone() }
    }
}

impl<T, O> PoolSubscription<T, O>
where
    T: PoolTransaction,
    O: PoolOrd<Transaction = T>,
{
    pub(crate) fn broadcast(&self, tx: PendingTx<T, O>) {
        self.notify.notify_waiters();
        self.txs.write().insert(tx);
    }
}

impl<T, O> Stream for PoolSubscription<T, O>
where
    T: PoolTransaction,
    O: PoolOrd<Transaction = T>,
{
    type Item = PendingTx<T, O>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if let Some(tx) = this.txs.write().pop_first() {
                return Poll::Ready(Some(tx));
            }

            if pin!(this.notify.notified()).poll(cx).is_pending() {
                break;
            }
        }

        Poll::Pending
    }
}
