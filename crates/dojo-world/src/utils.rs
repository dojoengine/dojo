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
///
///
/// # Arguments
///
/// * `tx_hash` - The hash of the transaction to wait for.
/// * `status` - The status to wait for. Defaults to `TransactionStatus::AcceptedOnL2`.
/// * `interval` - Poll the transaction every `interval` miliseconds. Miliseconds are used so that
///   we can be more precise with the polling interval. Defaults to 1 second.
/// * `timeout` - The maximum amount of time to wait for the transaction to achieve `status` status.
///   Defaults to 60 seconds.
/// * `provider` - The provider to use for polling the transaction.
///
///
///  # Examples
///
/// ```
/// let provider = JsonRpcClient::new(HttpTransport::new("http://localhost:5000").unwrap());
/// let tx_hash = FieldElement::from_hex_str("0x1234").unwrap();
/// TransactionWaiter::new(tx_hash, provider).await;
/// ```
pub struct TransactionWaiter<'a, P>
where
    P: Provider,
{
    tx_hash: FieldElement,
    status: TransactionStatus,
    interval: Interval,
    timeout: Duration,
    provider: &'a P,
    /// The future that get the transaction receipt.
    future: Option<
        Pin<
            Box<
                dyn Future<
                        Output = Result<
                            MaybePendingTransactionReceipt,
                            ProviderError<<P as Provider>::Error>,
                        >,
                    > + Send
                    + 'a,
            >,
        >,
    >,
}

impl<'a, P> TransactionWaiter<'a, P>
where
    P: Provider + Send,
{
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
    const DEFAULT_INTERVAL: Duration = Duration::from_millis(200);
    const DEFAULT_STATUS: TransactionStatus = TransactionStatus::AcceptedOnL2;

    pub fn new(tx: FieldElement, provider: &'a P) -> Self {
        Self {
            provider,
            tx_hash: tx,
            future: None,
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
}

impl<'a, P> Future for TransactionWaiter<'a, P>
where
    P: Provider + Send,
{
    type Output = Result<TransactionReceipt, TransactionWaitingError<P::Error>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let elapsed = Instant::now();

        loop {
            if elapsed.elapsed() > this.timeout {
                return Poll::Ready(Err(TransactionWaitingError::Timeout));
            }

            if let Some(mut flush) = this.future.take() {
                match flush.poll_unpin(cx) {
                    Poll::Ready(res) => match res {
                        Ok(MaybePendingTransactionReceipt::Receipt(receipt)) => {
                            match transaction_status_from_receipt(&receipt) {
                                TransactionStatus::Rejected => {
                                    return Poll::Ready(Err(
                                        TransactionWaitingError::TransactionRejected,
                                    ))
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
