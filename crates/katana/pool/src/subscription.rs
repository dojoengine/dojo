use std::collections::BTreeSet;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{FutureExt, Stream};
use parking_lot::Mutex;
use tokio::sync::Notify;

use crate::ordering::PoolOrd;
use crate::tx::PendingTx;
use crate::TransactionPool;

pub struct PoolSubscription<T, O>
where
    T: TransactionPool,
    O: PoolOrd,
{
    notification: Notify,
    transactions: Mutex<BTreeSet<PendingTx<T, O>>>,
}

impl<T, O> PoolSubscription<T, O>
where
    T: TransactionPool,
    O: PoolOrd,
{
    pub fn new() -> Self {
        Self { notification: Notify::new(), transactions: Mutex::new(BTreeSet::new()) }
    }
}

impl<T, O> Stream for PoolSubscription<T, O>
where
    T: TransactionPool,
    O: PoolOrd,
{
    type Item = PendingTx<T, O>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(tx) = this.transactions.lock().pop_first() {
            return Poll::Ready(Some(tx));
        } else {
            let _ = Box::pin(this.notification.notified()).poll_unpin(cx);
        }

        Poll::Pending
    }
}
