use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::FutureExt;
use starknet::core::types::{
    FieldElement, MaybePendingTransactionReceipt, StarknetError, TransactionReceipt,
    TransactionStatus,
};
use starknet::providers::{Provider, ProviderError};
use tokio::time::{Instant, Interval};

type GetReceiptResult<E> = Result<MaybePendingTransactionReceipt, ProviderError<E>>;
type GetReceiptFuture<'a, E> = Pin<Box<dyn Future<Output = GetReceiptResult<E>> + Send + 'a>>;

#[derive(Debug, thiserror::Error)]
pub enum TransactionWaitingError<E> {
    #[error("request timed out")]
    Timeout,
    #[error("transaction was rejected")]
    TransactionRejected,
    #[error(transparent)]
    Provider(ProviderError<E>),
}

/// A type that waits for a transaction to achieve `status` status. The transaction will be polled
/// for every `interval` miliseconds. If the transaction does not achieved `status` status within
/// `timeout` miliseconds, an error will be returned. An error is also returned if the transaction
/// is rejected ( i.e., the transaction returns a `REJECTED` status ).
pub struct TransactionWaiter<'a, P>
where
    P: Provider,
{
    /// The hash of the transaction to wait for.
    tx_hash: FieldElement,
    /// The status to wait for. Defaults to `TransactionStatus::AcceptedOnL2`.
    status: TransactionStatus,
    /// Poll the transaction every `interval` miliseconds. Miliseconds are used so that
    /// we can be more precise with the polling interval. Defaults to 250ms.
    interval: Interval,
    /// The maximum amount of time to wait for the transaction to achieve `status` status.
    /// Defaults to 60 seconds.
    timeout: Duration,
    /// The provider to use for polling the transaction.
    provider: &'a P,
    /// The future that get the transaction receipt.
    future: Option<GetReceiptFuture<'a, <P as Provider>::Error>>,
    /// The time when the transaction waiter was polled.
    started_at: Option<Instant>,
}

impl<'a, P> TransactionWaiter<'a, P>
where
    P: Provider + Send,
{
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
    const DEFAULT_INTERVAL: Duration = Duration::from_millis(250);
    const DEFAULT_STATUS: TransactionStatus = TransactionStatus::AcceptedOnL2;

    pub fn new(tx: FieldElement, provider: &'a P) -> Self {
        Self {
            provider,
            tx_hash: tx,
            future: None,
            started_at: None,
            status: Self::DEFAULT_STATUS,
            timeout: Self::DEFAULT_TIMEOUT,
            interval: tokio::time::interval_at(
                Instant::now() + Self::DEFAULT_INTERVAL,
                Self::DEFAULT_INTERVAL,
            ),
        }
    }

    pub fn with_interval(mut self, milisecond: u64) -> Self {
        let interval = Duration::from_millis(milisecond);
        self.interval = tokio::time::interval_at(Instant::now() + interval, interval);
        self
    }

    pub fn with_status(mut self, status: TransactionStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl<'a, P> Future for TransactionWaiter<'a, P>
where
    P: Provider + Send,
{
    type Output = Result<TransactionReceipt, TransactionWaitingError<P::Error>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        if this.started_at.is_none() {
            this.started_at = Some(Instant::now());
        }

        loop {
            if let Some(started_at) = this.started_at {
                if started_at.elapsed() > this.timeout {
                    return Poll::Ready(Err(TransactionWaitingError::Timeout));
                }
            }

            if let Some(mut flush) = this.future.take() {
                match flush.poll_unpin(cx) {
                    Poll::Ready(res) => match res {
                        Ok(MaybePendingTransactionReceipt::Receipt(receipt)) => {
                            match transaction_status_from_receipt(&receipt) {
                                TransactionStatus::Rejected => {
                                    return Poll::Ready(Err(
                                        TransactionWaitingError::TransactionRejected,
                                    ));
                                }

                                status if status == this.status => return Poll::Ready(Ok(receipt)),

                                _ => {}
                            }
                        }

                        Ok(MaybePendingTransactionReceipt::PendingReceipt(_))
                        | Err(ProviderError::StarknetError(
                            StarknetError::TransactionHashNotFound,
                        )) => {}

                        Err(e) => return Poll::Ready(Err(TransactionWaitingError::Provider(e))),
                    },

                    Poll::Pending => {
                        this.future = Some(flush);
                        return Poll::Pending;
                    }
                }
            }

            if this.interval.poll_tick(cx).is_ready() {
                this.future = Some(Box::pin(this.provider.get_transaction_receipt(this.tx_hash)));
            } else {
                break;
            }
        }

        Poll::Pending
    }
}

fn transaction_status_from_receipt(receipt: &TransactionReceipt) -> TransactionStatus {
    match receipt {
        TransactionReceipt::Invoke(receipt) => receipt.status,
        TransactionReceipt::Deploy(receipt) => receipt.status,
        TransactionReceipt::Declare(receipt) => receipt.status,
        TransactionReceipt::L1Handler(receipt) => receipt.status,
        TransactionReceipt::DeployAccount(receipt) => receipt.status,
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use dojo_test_utils::sequencer::{SequencerConfig, TestSequencer};
    use starknet::core::types::FieldElement;
    use starknet::providers::jsonrpc::HttpTransport;
    use starknet::providers::JsonRpcClient;

    use super::{Duration, TransactionWaiter};

    #[tokio::test]
    async fn should_timeout_on_nonexistant_transaction() {
        let sequencer = TestSequencer::start(SequencerConfig::default()).await;
        let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));
        assert_matches!(
            TransactionWaiter::new(FieldElement::from_hex_be("0x1234").unwrap(), &provider)
                .with_timeout(Duration::from_secs(1))
                .await,
            Err(super::TransactionWaitingError::Timeout)
        );
    }
}
