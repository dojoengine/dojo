use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::FutureExt;
use starknet::core::types::{
    ExecutionResult, FieldElement, MaybePendingTransactionReceipt, PendingTransactionReceipt,
    StarknetError, TransactionFinalityStatus, TransactionReceipt, TransactionStatus,
};
use starknet::providers::{Provider, ProviderError};
use tokio::time::{Instant, Interval};

type GetTxStatusResult = Result<TransactionStatus, ProviderError>;
type GetTxReceiptResult = Result<MaybePendingTransactionReceipt, ProviderError>;

type GetTxStatusFuture<'a> = Pin<Box<dyn Future<Output = GetTxStatusResult> + Send + 'a>>;
type GetTxReceiptFuture<'a> = Pin<Box<dyn Future<Output = GetTxReceiptResult> + Send + 'a>>;

#[derive(Debug, thiserror::Error)]
pub enum TransactionWaitingError {
    #[error("request timed out")]
    Timeout,
    #[error("transaction reverted with reason: {0}")]
    TransactionReverted(String),
    #[error("transaction rejected")]
    TransactionRejected,
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
    /// The transaction finality status to wait for.
    ///
    /// If not set, then it will wait until it is `ACCEPTED_ON_L2` whether it is reverted or not.
    tx_finality_status: Option<TransactionFinalityStatus>,
    /// A flag to indicate that the waited transaction must either be successfully executed or not.
    ///
    /// If it's set to `true`, then the transaction execution result must be `SUCCEEDED` otherwise
    /// an error will be returned. However, if set to `false`, then the execution status will not
    /// be considered when waiting for the transaction, meaning `REVERTED` transaction will not
    /// return an error.
    must_succeed: bool,
    /// Poll the transaction every `interval` milliseconds. Milliseconds are used so that
    /// we can be more precise with the polling interval. Defaults to 2.5 seconds.
    interval: Interval,
    /// The maximum amount of time to wait for the transaction to achieve the desired status. An
    /// error will be returned if it is unable to finish within the `timeout` duration. Defaults to
    /// 300 seconds.
    timeout: Duration,
    /// The provider to use for polling the transaction.
    provider: &'a P,
    /// The future that gets the transaction status.
    tx_status_request_fut: Option<GetTxStatusFuture<'a>>,
    /// The future that gets the transaction receipt.
    tx_receipt_request_fut: Option<GetTxReceiptFuture<'a>>,
    /// The time when the transaction waiter was first polled.
    started_at: Option<Instant>,
}

impl<'a, P> TransactionWaiter<'a, P>
where
    P: Provider + Send,
{
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
    /// Interval for use with 3rd party provider without burning the API rate limit.
    const DEFAULT_INTERVAL: Duration = Duration::from_millis(2500);

    pub fn new(tx: FieldElement, provider: &'a P) -> Self {
        Self {
            provider,
            tx_hash: tx,
            started_at: None,
            must_succeed: true,
            tx_finality_status: None,
            tx_status_request_fut: None,
            tx_receipt_request_fut: None,
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

    pub fn with_tx_status(self, status: TransactionFinalityStatus) -> Self {
        Self { tx_finality_status: Some(status), ..self }
    }

    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    // Helper function to evaluate if the transaction receipt should be accepted yet or not, based
    // on the waiter's parameters. Used in the `Future` impl.
    fn evaluate_receipt_from_params(
        receipt: MaybePendingTransactionReceipt,
        expected_finality_status: Option<TransactionFinalityStatus>,
        must_succeed: bool,
    ) -> Option<Result<MaybePendingTransactionReceipt, TransactionWaitingError>> {
        match &receipt {
            MaybePendingTransactionReceipt::PendingReceipt(r) => {
                // pending receipt doesn't include finality status, so we cant check it.
                if expected_finality_status.is_some() {
                    return None;
                }

                if !must_succeed {
                    return Some(Ok(receipt));
                }

                match execution_status_from_pending_receipt(r) {
                    ExecutionResult::Succeeded => Some(Ok(receipt)),
                    ExecutionResult::Reverted { reason } => {
                        Some(Err(TransactionWaitingError::TransactionReverted(reason.clone())))
                    }
                }
            }

            MaybePendingTransactionReceipt::Receipt(r) => {
                if let Some(expected_status) = expected_finality_status {
                    match finality_status_from_receipt(r) {
                        TransactionFinalityStatus::AcceptedOnL2
                            if expected_status == TransactionFinalityStatus::AcceptedOnL1 =>
                        {
                            None
                        }

                        _ => {
                            if !must_succeed {
                                return Some(Ok(receipt));
                            }

                            match execution_status_from_receipt(r) {
                                ExecutionResult::Succeeded => Some(Ok(receipt)),
                                ExecutionResult::Reverted { reason } => Some(Err(
                                    TransactionWaitingError::TransactionReverted(reason.clone()),
                                )),
                            }
                        }
                    }
                } else {
                    Some(Ok(receipt))
                }
            }
        }
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

            if let Some(mut fut) = this.tx_status_request_fut.take() {
                match fut.poll_unpin(cx) {
                    Poll::Ready(res) => match res {
                        Ok(status) => match status {
                            TransactionStatus::AcceptedOnL2(_)
                            | TransactionStatus::AcceptedOnL1(_) => {
                                this.tx_receipt_request_fut = Some(Box::pin(
                                    this.provider.get_transaction_receipt(this.tx_hash),
                                ));
                            }

                            TransactionStatus::Rejected => {
                                return Poll::Ready(Err(
                                    TransactionWaitingError::TransactionRejected,
                                ));
                            }

                            TransactionStatus::Received => {}
                        },

                        Err(ProviderError::StarknetError(
                            StarknetError::TransactionHashNotFound,
                        )) => {}

                        Err(e) => {
                            return Poll::Ready(Err(TransactionWaitingError::Provider(e)));
                        }
                    },

                    Poll::Pending => {
                        this.tx_status_request_fut = Some(fut);
                        return Poll::Pending;
                    }
                }
            }

            if let Some(mut fut) = this.tx_receipt_request_fut.take() {
                match fut.poll_unpin(cx) {
                    Poll::Pending => {
                        this.tx_receipt_request_fut = Some(fut);
                        return Poll::Pending;
                    }

                    Poll::Ready(res) => match res {
                        Err(ProviderError::StarknetError(
                            StarknetError::TransactionHashNotFound,
                        )) => {}

                        Err(e) => {
                            return Poll::Ready(Err(TransactionWaitingError::Provider(e)));
                        }

                        Ok(receipt) => {
                            if let Some(res) = Self::evaluate_receipt_from_params(
                                receipt,
                                this.tx_finality_status,
                                this.must_succeed,
                            ) {
                                return Poll::Ready(res);
                            }
                        }
                    },
                }
            }

            if this.interval.poll_tick(cx).is_ready() {
                this.tx_status_request_fut =
                    Some(Box::pin(this.provider.get_transaction_status(this.tx_hash)));
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
    use starknet::core::types::{
        ExecutionResources, ExecutionResult, FeePayment, FieldElement, InvokeTransactionReceipt,
        MaybePendingTransactionReceipt, PendingInvokeTransactionReceipt, PendingTransactionReceipt,
        PriceUnit, TransactionFinalityStatus, TransactionReceipt,
    };
    use starknet::providers::jsonrpc::HttpTransport;
    use starknet::providers::JsonRpcClient;

    use super::{Duration, TransactionWaiter};

    async fn create_test_sequencer() -> (TestSequencer, JsonRpcClient<HttpTransport>) {
        let sequencer =
            TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config())
                .await;
        let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));
        (sequencer, provider)
    }

    const EXECUTION_RESOURCES: ExecutionResources = ExecutionResources {
        steps: 0,
        memory_holes: None,
        ec_op_builtin_applications: Some(0),
        ecdsa_builtin_applications: Some(0),
        keccak_builtin_applications: Some(0),
        bitwise_builtin_applications: Some(0),
        pedersen_builtin_applications: Some(0),
        poseidon_builtin_applications: Some(0),
        range_check_builtin_applications: Some(0),
        segment_arena_builtin: Some(0),
    };

    fn mock_receipt(
        finality_status: TransactionFinalityStatus,
        execution_result: ExecutionResult,
    ) -> TransactionReceipt {
        TransactionReceipt::Invoke(InvokeTransactionReceipt {
            finality_status,
            execution_result,
            events: Default::default(),
            actual_fee: FeePayment { amount: Default::default(), unit: PriceUnit::Wei },
            block_hash: Default::default(),
            block_number: Default::default(),
            messages_sent: Default::default(),
            transaction_hash: Default::default(),
            execution_resources: EXECUTION_RESOURCES,
        })
    }

    fn mock_pending_receipt(execution_result: ExecutionResult) -> PendingTransactionReceipt {
        PendingTransactionReceipt::Invoke(PendingInvokeTransactionReceipt {
            execution_result,
            events: Default::default(),
            actual_fee: FeePayment { amount: Default::default(), unit: PriceUnit::Wei },
            messages_sent: Default::default(),
            transaction_hash: Default::default(),
            execution_resources: EXECUTION_RESOURCES,
        })
    }

    #[tokio::test]
    async fn should_timeout_on_nonexistant_transaction() {
        let (_sequencer, provider) = create_test_sequencer().await;

        assert_matches!(
            TransactionWaiter::new(FieldElement::from_hex_be("0x1234").unwrap(), &provider)
                .with_timeout(Duration::from_secs(1))
                .await,
            Err(super::TransactionWaitingError::Timeout)
        );
    }

    #[test]
    fn wait_for_no_finality_status() {
        let receipt = MaybePendingTransactionReceipt::Receipt(mock_receipt(
            TransactionFinalityStatus::AcceptedOnL2,
            ExecutionResult::Succeeded,
        ));

        assert_eq!(
            TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                receipt.clone(),
                None,
                false
            )
            .unwrap()
            .unwrap(),
            receipt
        );
    }

    macro_rules! assert_eval_receipt {
        (($receipt:expr, $expected_status:expr), $expected_receipt:expr) => {
            assert_eq!(
                TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                    $receipt,
                    $expected_status,
                    false
                )
                .unwrap()
                .unwrap(),
                $expected_receipt
            );
        };
    }

    #[test]
    fn wait_for_finality_status_with_no_succeed() {
        {
            let receipt = MaybePendingTransactionReceipt::Receipt(mock_receipt(
                TransactionFinalityStatus::AcceptedOnL2,
                ExecutionResult::Succeeded,
            ));

            assert_eval_receipt!(
                (receipt.clone(), Some(TransactionFinalityStatus::AcceptedOnL2)),
                receipt
            );
        }

        {
            let receipt = MaybePendingTransactionReceipt::Receipt(mock_receipt(
                TransactionFinalityStatus::AcceptedOnL2,
                ExecutionResult::Succeeded,
            ));

            assert!(
                TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                    receipt,
                    Some(TransactionFinalityStatus::AcceptedOnL1),
                    true,
                )
                .is_none()
            );
        }

        {
            let receipt = MaybePendingTransactionReceipt::Receipt(mock_receipt(
                TransactionFinalityStatus::AcceptedOnL1,
                ExecutionResult::Succeeded,
            ));

            assert_eval_receipt!(
                (receipt.clone(), Some(TransactionFinalityStatus::AcceptedOnL2)),
                receipt
            );
        }

        {
            let receipt = MaybePendingTransactionReceipt::Receipt(mock_receipt(
                TransactionFinalityStatus::AcceptedOnL1,
                ExecutionResult::Succeeded,
            ));

            assert_eval_receipt!(
                (receipt.clone(), Some(TransactionFinalityStatus::AcceptedOnL1)),
                receipt
            );
        }
    }

    #[test]
    fn wait_for_finality_status_with_must_succeed() {
        {
            let receipt = MaybePendingTransactionReceipt::Receipt(mock_receipt(
                TransactionFinalityStatus::AcceptedOnL2,
                ExecutionResult::Succeeded,
            ));

            assert_eq!(
                TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                    receipt.clone(),
                    Some(TransactionFinalityStatus::AcceptedOnL2),
                    true
                )
                .unwrap()
                .unwrap(),
                receipt
            )
        }

        {
            let receipt = MaybePendingTransactionReceipt::Receipt(mock_receipt(
                TransactionFinalityStatus::AcceptedOnL1,
                ExecutionResult::Succeeded,
            ));

            assert_eq!(
                TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                    receipt.clone(),
                    Some(TransactionFinalityStatus::AcceptedOnL2),
                    true
                )
                .unwrap()
                .unwrap(),
                receipt
            )
        }

        {
            let receipt = MaybePendingTransactionReceipt::Receipt(mock_receipt(
                TransactionFinalityStatus::AcceptedOnL1,
                ExecutionResult::Reverted { reason: Default::default() },
            ));

            let err =
                TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                    receipt,
                    Some(TransactionFinalityStatus::AcceptedOnL1),
                    true,
                )
                .unwrap()
                .unwrap_err();

            assert!(err.to_string().contains("transaction reverted"))
        }
    }

    #[test]
    fn wait_for_pending_tx() {
        {
            let receipt = MaybePendingTransactionReceipt::PendingReceipt(mock_pending_receipt(
                ExecutionResult::Succeeded,
            ));

            assert!(
                TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                    receipt,
                    Some(TransactionFinalityStatus::AcceptedOnL2),
                    true
                )
                .is_none()
            )
        }

        {
            let receipt = MaybePendingTransactionReceipt::PendingReceipt(mock_pending_receipt(
                ExecutionResult::Reverted { reason: Default::default() },
            ));

            assert_eq!(
                TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                    receipt.clone(),
                    None,
                    false
                )
                .unwrap()
                .unwrap(),
                receipt
            )
        }

        {
            let receipt = MaybePendingTransactionReceipt::PendingReceipt(mock_pending_receipt(
                ExecutionResult::Reverted { reason: Default::default() },
            ));

            let err =
                TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                    receipt, None, true,
                )
                .unwrap()
                .unwrap_err();

            assert!(err.to_string().contains("transaction reverted"))
        }
    }
}
