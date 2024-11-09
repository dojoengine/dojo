use std::collections::btree_set::IntoIter;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Stream, StreamExt};

use crate::ordering::PoolOrd;
use crate::subscription::Subscription;
use crate::tx::{PendingTx, PoolTransaction};

/// An iterator that yields transactions from the pool that can be included in a block, sorted by
/// by its priority.
#[derive(Debug)]
pub struct PendingTransactions<T, O: PoolOrd> {
    /// Iterator over all the pending transactions at the time of the creation of this struct.
    pub(crate) all: IntoIter<PendingTx<T, O>>,
    /// Subscription to the pool to get notified when new transactions are added. This is used to
    /// wait on the new transactions after exhausting the `all` iterator.
    pub(crate) subscription: Subscription<T, O>,
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

#[cfg(test)]
mod tests {

    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use futures::StreamExt;
    use tokio::task::yield_now;

    use crate::pool::test_utils::PoolTx;
    use crate::pool::Pool;
    use crate::validation::NoopValidator;
    use crate::{ordering, PoolTransaction, TransactionPool};

    #[tokio::test]
    async fn pending_transactions() {
        let pool = Pool::new(NoopValidator::<PoolTx>::new(), ordering::FiFo::new());

        let first_batch = [
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
        ];

        for tx in &first_batch {
            pool.add_transaction(tx.clone()).expect("failed to add tx");
        }

        let mut pendings = pool.pending_transactions();

        // exhaust all the first batch transactions
        for expected in &first_batch {
            let actual = pendings.next().await.map(|t| t.tx).unwrap();
            assert_eq!(expected, actual.as_ref());
        }

        let second_batch = [
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
            PoolTx::new(),
        ];

        for tx in &second_batch {
            pool.add_transaction(tx.clone()).expect("failed to add tx");
        }

        // exhaust all the first batch transactions
        for expected in &second_batch {
            let actual = pendings.next().await.map(|t| t.tx).unwrap();
            assert_eq!(expected, actual.as_ref());
        }

        // Check that all the added transaction is still in the pool because we haven't removed it
        // yet.
        let all = [first_batch, second_batch].concat();
        for tx in all {
            assert!(pool.contains(tx.hash()));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscription_stream_wakeup() {
        let pool = Pool::new(NoopValidator::<PoolTx>::new(), ordering::FiFo::new());
        let mut pending = pool.pending_transactions();

        // Spawn a task that will add a transaction after a delay
        let pool_clone = pool.clone();

        let txs = [PoolTx::new(), PoolTx::new(), PoolTx::new()];
        let txs_clone = txs.clone();

        let has_polled_once = Arc::new(AtomicBool::new(false));
        let has_polled_once_clone = has_polled_once.clone();

        tokio::spawn(async move {
            while !has_polled_once_clone.load(Ordering::SeqCst) {
                yield_now().await;
            }

            for tx in txs_clone {
                pool_clone.add_transaction(tx).expect("failed to add tx");
            }
        });

        // Check that first poll_next returns Pending because no pending transaction has been added
        // to the pool yet
        assert!(futures_util::poll!(pending.next()).is_pending());
        has_polled_once.store(true, Ordering::SeqCst);

        for tx in txs {
            let received = pending.next().await.unwrap();
            assert_eq!(&tx, received.tx.as_ref());
        }
    }
}
