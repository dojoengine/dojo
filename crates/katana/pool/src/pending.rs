use std::collections::btree_set::IntoIter;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Stream, StreamExt};

use crate::ordering::PoolOrd;
use crate::subscription::PoolSubscription;
use crate::tx::{PendingTx, PoolTransaction};

/// an iterator that yields transactions from the pool that can be included in a block, sorted by
/// by its priority.
#[derive(Debug)]
pub struct PendingTransactions<T, O: PoolOrd> {
    pub(crate) all: IntoIter<PendingTx<T, O>>,
    pub(crate) subscription: PoolSubscription<T, O>,
}

impl<T, O> Stream for PendingTransactions<T, O>
where
    T: PoolTransaction,
    O: PoolOrd<Transaction = T>,
{
    type Item = PendingTx<T, O>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(tx) = this.all.next() {
            Poll::Ready(Some(tx))
        } else {
            this.subscription.poll_next_unpin(cx)
        }
    }
}

impl<T, O> Iterator for PendingTransactions<T, O>
where
    T: PoolTransaction,
    O: PoolOrd<Transaction = T>,
{
    type Item = PendingTx<T, O>;

    fn next(&mut self) -> Option<Self::Item> {
        self.all.next()
    }
}
