use std::collections::BTreeSet;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use crate::ordering::PoolOrd;
use crate::tx::{PendingTx, PoolTransaction};

/// A subscription to newly added transactions in the pool.
///
/// This stream yields transactions in the order determined by the pool ordering, even if multiple
/// transactions are received between polls.
#[derive(Debug)]
pub struct Subscription<T, O: PoolOrd> {
    txs: Mutex<BTreeSet<PendingTx<T, O>>>,
    receiver: mpsc::UnboundedReceiver<PendingTx<T, O>>,
}

impl<T, O> Subscription<T, O>
where
    T: PoolTransaction,
    O: PoolOrd<Transaction = T>,
{
    pub(crate) fn new(receiver: mpsc::UnboundedReceiver<PendingTx<T, O>>) -> Self {
        Self { txs: Default::default(), receiver }
    }
}

impl<T, O> Stream for Subscription<T, O>
where
    T: PoolTransaction,
    O: PoolOrd<Transaction = T>,
{
    type Item = PendingTx<T, O>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let mut txs = this.txs.lock();

        // In the event where a lot of transactions have been sent to the receiver channel and this
        // stream hasn't been iterated since, the next call to `.next()` of this Stream will
        // require to drain the channel and insert all the transactions into the btree set. If there
        // are a lot of transactions to insert, it would take a while and might block the
        // runtime.
        loop {
            if let Some(tx) = txs.pop_first() {
                return Poll::Ready(Some(tx));
            }

            // Check the channel if there are new transactions available.
            match this.receiver.poll_recv(cx) {
                // insert the new transactions into the btree set to make sure they are ordered
                // according to the pool's ordering.
                Poll::Ready(Some(tx)) => {
                    txs.insert(tx);

                    // Check if there are more transactions available in the channel.
                    while let Poll::Ready(Some(tx)) = this.receiver.poll_recv(cx) {
                        txs.insert(tx);
                    }
                }

                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
