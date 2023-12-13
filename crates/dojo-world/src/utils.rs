use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::FutureExt;
use starknet::core::types::{
    ExecutionResult, FieldElement, MaybePendingTransactionReceipt, PendingTransactionReceipt,
    StarknetError, TransactionFinalityStatus, TransactionReceipt,
};
use starknet::providers::{Provider, ProviderError};
use tokio::time::{Instant, Interval};

type GetReceiptResult = Result<MaybePendingTransactionReceipt, ProviderError>;
type GetReceiptFuture<'a> = Pin<Box<dyn Future<Output = GetReceiptResult> + Send + 'a>>;

#[derive(Debug, thiserror::Error)]
pub enum TransactionWaitingError {
    #[error("request timed out")]
    Timeout,
    #[error("transaction reverted due to failed execution: {0}")]
    TransactionReverted(String),
    #[error(transparent)]
    Provider(ProviderError),
}

/// A type that waits for a transaction to achieve the desired status. The waiter will poll for the
/// transaction receipt every `interval` miliseconds until it achieves the desired status or until
/// `timeout` is reached.
///
/// The waiter can be configured to wait for a specific finality status (e.g, `ACCEPTED_ON_L2`), by
/// default, it only waits until the transaction is included in the _pending_ block. It can also be
/// set to check if the transaction is executed successfully or not (reverted).
///
/// # Examples
///
/// ```ignore
/// ues url::Url;
/// use starknet::providers::jsonrpc::HttpTransport;
/// use starknet::providers::JsonRpcClient;
/// use starknet::core::types::TransactionFinalityStatus;
///
/// let provider = JsonRpcClient::new(HttpTransport::new(Url::parse("http://localhost:5000").unwrap()));
///
/// let tx_hash = FieldElement::from(0xbadbeefu64);
/// let receipt = TransactionWaiter::new(tx_hash, &provider).with_finality(TransactionFinalityStatus::ACCEPTED_ON_L2).await.unwrap();
/// ```
#[must_use = "TransactionWaiter does nothing unless polled"]
pub struct TransactionWaiter<'a, P: Provider> {
    /// The hash of the transaction to wait for.
    tx_hash: FieldElement,
    /// The finality status to wait for.
    ///
    /// If set, the waiter will wait for the transaction to achieve this finality status.
    /// Otherwise, the waiter will only wait for the transaction until it is included in the
    /// _pending_ block.
    finality_status: Option<TransactionFinalityStatus>,
    /// A flag to indicate that the waited transaction must either be successfully executed or not.
    ///
    /// If it's set to `true`, then the transaction execution status must be `SUCCEEDED` otherwise
    /// an error will be returned. However, if set to `false`, then the execution status will not
    /// be considered when waiting for the transaction, meaning `REVERTED` transaction will not
    /// return an error.
    must_succeed: bool,
    /// Poll the transaction every `interval` miliseconds. Miliseconds are used so that
    /// we can be more precise with the polling interval. Defaults to 250ms.
    interval: Interval,
    /// The maximum amount of time to wait for the transaction to achieve the desired status. An
    /// error will be returned if it is unable to finish within the `timeout` duration. Defaults to
    /// 60 seconds.
    timeout: Duration,
    /// The provider to use for polling the transaction.
    provider: &'a P,
    /// The future that gets the transaction receipt.
    receipt_request_fut: Option<GetReceiptFuture<'a>>,
    /// The time when the transaction waiter was first polled.
    started_at: Option<Instant>,
}

impl<'a, P> TransactionWaiter<'a, P>
where
    P: Provider + Send,
{
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
    const DEFAULT_INTERVAL: Duration = Duration::from_millis(2500);

    pub fn new(tx: FieldElement, provider: &'a P) -> Self {
        Self {
            provider,
            tx_hash: tx,
            started_at: None,
            must_succeed: true,
            finality_status: None,
            receipt_request_fut: None,
            timeout: Self::DEFAULT_TIMEOUT,
            interval: tokio::time::interval_at(
                Instant::now() + Self::DEFAULT_INTERVAL,
                Self::DEFAULT_INTERVAL,
            ),
        }
    }

    pub fn with_interval(self, milisecond: u64) -> Self {
        let interval = Duration::from_millis(milisecond);
        Self { interval: tokio::time::interval_at(Instant::now() + interval, interval), ..self }
    }

    pub fn with_finality(self, status: TransactionFinalityStatus) -> Self {
        Self { finality_status: Some(status), ..self }
    }

    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }
}

impl<'a, P> Future for TransactionWaiter<'a, P>
where
    P: Provider + Send,
{
    type Output = Result<MaybePendingTransactionReceipt, TransactionWaitingError>;

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

            if let Some(mut flush) = this.receipt_request_fut.take() {
                match flush.poll_unpin(cx) {
                    Poll::Ready(res) => match res {
                        Ok(receipt) => match &receipt {
                            MaybePendingTransactionReceipt::PendingReceipt(r) => {
                                if this.finality_status.is_none() {
                                    if this.must_succeed {
                                        let res = match execution_status_from_pending_receipt(r) {
                                            ExecutionResult::Succeeded => Ok(receipt),
                                            ExecutionResult::Reverted { reason } => {
                                                Err(TransactionWaitingError::TransactionReverted(
                                                    reason.clone(),
                                                ))
                                            }
                                        };
                                        return Poll::Ready(res);
                                    }

                                    return Poll::Ready(Ok(receipt));
                                }
                            }

                            MaybePendingTransactionReceipt::Receipt(r) => {
                                if let Some(finality_status) = this.finality_status {
                                    match finality_status_from_receipt(r) {
                                        status if status == finality_status => {
                                            if this.must_succeed {
                                                let res = match execution_status_from_receipt(r) {
                                                    ExecutionResult::Succeeded => Ok(receipt),
                                                    ExecutionResult::Reverted { reason } => {
                                                        Err(TransactionWaitingError::TransactionReverted(
                                                            reason.clone(),
                                                        ))
                                                    }
                                                };
                                                return Poll::Ready(res);
                                            }

                                            return Poll::Ready(Ok(receipt));
                                        }

                                        _ => {}
                                    }
                                } else {
                                    return Poll::Ready(Ok(receipt));
                                }
                            }
                        },

                        Err(ProviderError::StarknetError(
                            StarknetError::TransactionHashNotFound,
                        )) => {}

                        Err(e) => {
                            return Poll::Ready(Err(TransactionWaitingError::Provider(e)));
                        }
                    },

                    Poll::Pending => {
                        this.receipt_request_fut = Some(flush);
                        return Poll::Pending;
                    }
                }
            }

            if this.interval.poll_tick(cx).is_ready() {
                this.receipt_request_fut =
                    Some(Box::pin(this.provider.get_transaction_receipt(this.tx_hash)));
            } else {
                break;
            }
        }

        Poll::Pending
    }
}

#[inline]
fn execution_status_from_receipt(receipt: &TransactionReceipt) -> &ExecutionResult {
    match receipt {
        TransactionReceipt::Invoke(receipt) => &receipt.execution_result,
        TransactionReceipt::Deploy(receipt) => &receipt.execution_result,
        TransactionReceipt::Declare(receipt) => &receipt.execution_result,
        TransactionReceipt::L1Handler(receipt) => &receipt.execution_result,
        TransactionReceipt::DeployAccount(receipt) => &receipt.execution_result,
    }
}

#[inline]
fn execution_status_from_pending_receipt(receipt: &PendingTransactionReceipt) -> &ExecutionResult {
    match receipt {
        PendingTransactionReceipt::Invoke(receipt) => &receipt.execution_result,
        PendingTransactionReceipt::Declare(receipt) => &receipt.execution_result,
        PendingTransactionReceipt::L1Handler(receipt) => &receipt.execution_result,
        PendingTransactionReceipt::DeployAccount(receipt) => &receipt.execution_result,
    }
}

#[inline]
fn finality_status_from_receipt(receipt: &TransactionReceipt) -> TransactionFinalityStatus {
    match receipt {
        TransactionReceipt::Invoke(receipt) => receipt.finality_status,
        TransactionReceipt::Deploy(receipt) => receipt.finality_status,
        TransactionReceipt::Declare(receipt) => receipt.finality_status,
        TransactionReceipt::L1Handler(receipt) => receipt.finality_status,
        TransactionReceipt::DeployAccount(receipt) => receipt.finality_status,
    }
}

#[inline]
pub fn block_number_from_receipt(receipt: &TransactionReceipt) -> u64 {
    match receipt {
        TransactionReceipt::Invoke(tx) => tx.block_number,
        TransactionReceipt::L1Handler(tx) => tx.block_number,
        TransactionReceipt::Declare(tx) => tx.block_number,
        TransactionReceipt::Deploy(tx) => tx.block_number,
        TransactionReceipt::DeployAccount(tx) => tx.block_number,
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use dojo_test_utils::sequencer::{
        get_default_test_starknet_config, SequencerConfig, TestSequencer,
    };
    use starknet::core::types::FieldElement;
    use starknet::providers::jsonrpc::HttpTransport;
    use starknet::providers::JsonRpcClient;

    use super::{Duration, TransactionWaiter};

    #[tokio::test]
    async fn should_timeout_on_nonexistant_transaction() {
        let sequencer =
            TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config())
                .await;
        let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));
        assert_matches!(
            TransactionWaiter::new(FieldElement::from_hex_be("0x1234").unwrap(), &provider)
                .with_timeout(Duration::from_secs(1))
                .await,
            Err(super::TransactionWaitingError::Timeout)
        );
    }
}
