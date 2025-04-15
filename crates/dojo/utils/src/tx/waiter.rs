use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::Result;
use futures::FutureExt;
use starknet::core::types::{
    ExecutionResult, Felt, ReceiptBlock, StarknetError, TransactionFinalityStatus,
    TransactionReceipt, TransactionReceiptWithBlockInfo, TransactionStatus,
};
use starknet::providers::{Provider, ProviderError};
use tokio::time::{Instant, Interval};

type GetTxStatusResult = Result<TransactionStatus, ProviderError>;
type GetTxReceiptResult = Result<TransactionReceiptWithBlockInfo, ProviderError>;

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

/// Utility for waiting on a transaction.
///
/// The waiter will poll for the transaction receipt every `interval` miliseconds until it achieves
/// the desired status or until `timeout` is reached.
///
/// The waiter can be configured to wait for a specific finality status (e.g, `ACCEPTED_ON_L2`), by
/// default, it only waits until the transaction is included in the _pending_ block. It can also be
/// set to check if the transaction is executed successfully or not (reverted).
///
/// # Examples
///
/// ```ignore
/// use url::Url;
/// use starknet::providers::jsonrpc::HttpTransport;
/// use starknet::providers::JsonRpcClient;
/// use starknet::core::types::TransactionFinalityStatus;
///
/// let provider = JsonRpcClient::new(HttpTransport::new(Url::parse("http://localhost:5000").unwrap()));
///
/// let tx_hash = Felt::from(0xbadbeefu64);
/// let receipt = TransactionWaiter::new(tx_hash, &provider).with_tx_status(TransactionFinalityStatus::AcceptedOnL2).await.unwrap();
/// ```
#[must_use = "TransactionWaiter does nothing unless polled"]
#[allow(missing_debug_implementations)]
pub struct TransactionWaiter<'a, P: Provider> {
    /// The hash of the transaction to wait for.
    tx_hash: Felt,
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
    /// The default timeout for a transaction to be accepted or reverted on L2.
    /// The inclusion (which can be accepted or reverted) is ~5seconds in ideal cases.
    /// We keep some margin for times that could be affected by network congestion or
    /// block STM worst cases.
    const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
    /// Interval for use with 3rd party provider without burning the API rate limit.
    const DEFAULT_INTERVAL: Duration = Duration::from_millis(2500);

    pub fn new(tx: Felt, provider: &'a P) -> Self {
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
        receipt: TransactionReceiptWithBlockInfo,
        expected_finality_status: Option<TransactionFinalityStatus>,
        must_succeed: bool,
    ) -> Option<Result<TransactionReceiptWithBlockInfo, TransactionWaitingError>> {
        match &receipt.block {
            ReceiptBlock::Pending => {
                // pending receipt doesn't include finality status, so we cant check it.
                if expected_finality_status.is_some() {
                    return None;
                }

                if !must_succeed {
                    return Some(Ok(receipt));
                }

                match execution_status_from_receipt(&receipt.receipt) {
                    ExecutionResult::Succeeded => Some(Ok(receipt)),
                    ExecutionResult::Reverted { reason } => {
                        Some(Err(TransactionWaitingError::TransactionReverted(reason.clone())))
                    }
                }
            }

            ReceiptBlock::Block { .. } => {
                if let Some(expected_status) = expected_finality_status {
                    match finality_status_from_receipt(&receipt.receipt) {
                        TransactionFinalityStatus::AcceptedOnL2
                            if expected_status == TransactionFinalityStatus::AcceptedOnL1 =>
                        {
                            None
                        }

                        _ => {
                            if !must_succeed {
                                return Some(Ok(receipt));
                            }

                            match execution_status_from_receipt(&receipt.receipt) {
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
    type Output = Result<TransactionReceiptWithBlockInfo, TransactionWaitingError>;

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

                        Ok(res) => {
                            if let Some(res) = Self::evaluate_receipt_from_params(
                                res,
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
pub fn execution_status_from_receipt(receipt: &TransactionReceipt) -> &ExecutionResult {
    match receipt {
        TransactionReceipt::Invoke(receipt) => &receipt.execution_result,
        TransactionReceipt::Deploy(receipt) => &receipt.execution_result,
        TransactionReceipt::Declare(receipt) => &receipt.execution_result,
        TransactionReceipt::L1Handler(receipt) => &receipt.execution_result,
        TransactionReceipt::DeployAccount(receipt) => &receipt.execution_result,
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

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use katana_runner::RunnerCtx;
    use starknet::core::types::ExecutionResult::{Reverted, Succeeded};
    use starknet::core::types::TransactionFinalityStatus::{self, AcceptedOnL1, AcceptedOnL2};
    use starknet::core::types::{
        ComputationResources, DataAvailabilityResources, DataResources, ExecutionResources,
        ExecutionResult, FeePayment, InvokeTransactionReceipt, PriceUnit, ReceiptBlock,
        TransactionReceipt, TransactionReceiptWithBlockInfo,
    };
    use starknet::macros::felt;
    use starknet::providers::jsonrpc::HttpTransport;
    use starknet::providers::JsonRpcClient;

    use super::{Duration, TransactionWaiter};
    use crate::TransactionWaitingError;

    const EXECUTION_RESOURCES: ExecutionResources = ExecutionResources {
        computation_resources: ComputationResources {
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
        },
        data_resources: DataResources {
            data_availability: DataAvailabilityResources { l1_gas: 0, l1_data_gas: 0 },
        },
    };

    fn mock_receipt(
        finality_status: TransactionFinalityStatus,
        execution_result: ExecutionResult,
    ) -> TransactionReceiptWithBlockInfo {
        let receipt = TransactionReceipt::Invoke(InvokeTransactionReceipt {
            finality_status,
            execution_result,
            events: Default::default(),
            actual_fee: FeePayment { amount: Default::default(), unit: PriceUnit::Wei },
            messages_sent: Default::default(),
            transaction_hash: Default::default(),
            execution_resources: EXECUTION_RESOURCES,
        });

        TransactionReceiptWithBlockInfo {
            receipt,
            block: ReceiptBlock::Block {
                block_hash: Default::default(),
                block_number: Default::default(),
            },
        }
    }

    fn mock_pending_receipt(execution_result: ExecutionResult) -> TransactionReceiptWithBlockInfo {
        let receipt = TransactionReceipt::Invoke(InvokeTransactionReceipt {
            execution_result,
            events: Default::default(),
            finality_status: TransactionFinalityStatus::AcceptedOnL2,
            actual_fee: FeePayment { amount: Default::default(), unit: PriceUnit::Wei },
            messages_sent: Default::default(),
            transaction_hash: Default::default(),
            execution_resources: EXECUTION_RESOURCES,
        });

        TransactionReceiptWithBlockInfo { receipt, block: ReceiptBlock::Pending }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[katana_runner::test(accounts = 10)]
    async fn should_timeout_on_nonexistant_transaction(sequencer: &RunnerCtx) {
        let provider = sequencer.provider();

        let hash = felt!("0x1234");
        let result = TransactionWaiter::new(hash, &provider)
            .with_timeout(Duration::from_secs(1))
            .await
            .unwrap_err();

        assert_matches!(result, TransactionWaitingError::Timeout);
    }

    macro_rules! eval_receipt {
        ($receipt:expr, $must_succeed:expr) => {
            TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                $receipt,
                None,
                $must_succeed,
            )
        };

        ($receipt:expr, $expected_status:expr, $must_succeed:expr) => {
            TransactionWaiter::<JsonRpcClient<HttpTransport>>::evaluate_receipt_from_params(
                $receipt,
                Some($expected_status),
                $must_succeed,
            )
        };
    }

    #[test]
    fn wait_for_no_finality_status() {
        let receipt = mock_receipt(AcceptedOnL2, Succeeded);
        assert!(eval_receipt!(receipt.clone(), false).unwrap().is_ok());
    }

    #[test]
    fn wait_for_finality_status_with_no_succeed() {
        let receipt = mock_receipt(AcceptedOnL2, Succeeded);
        assert!(eval_receipt!(receipt.clone(), AcceptedOnL2, false).unwrap().is_ok());

        let receipt = mock_receipt(AcceptedOnL2, Succeeded);
        assert!(eval_receipt!(receipt.clone(), AcceptedOnL1, true).is_none());

        let receipt = mock_receipt(AcceptedOnL1, Succeeded);
        assert!(eval_receipt!(receipt.clone(), AcceptedOnL2, false).unwrap().is_ok());

        let receipt = mock_receipt(AcceptedOnL1, Succeeded);
        assert!(eval_receipt!(receipt.clone(), AcceptedOnL1, false).unwrap().is_ok());
    }

    #[test]
    fn wait_for_finality_status_with_must_succeed() {
        let receipt = mock_receipt(AcceptedOnL2, Succeeded);
        assert!(eval_receipt!(receipt.clone(), AcceptedOnL2, true).unwrap().is_ok());

        let receipt = mock_receipt(AcceptedOnL1, Succeeded);
        assert!(eval_receipt!(receipt.clone(), AcceptedOnL2, true).unwrap().is_ok());

        let receipt = mock_receipt(AcceptedOnL1, Reverted { reason: Default::default() });
        let evaluation = eval_receipt!(receipt.clone(), AcceptedOnL1, true).unwrap();
        assert_matches!(evaluation, Err(TransactionWaitingError::TransactionReverted(_)));
    }

    #[test]
    fn wait_for_pending_tx() {
        let receipt = mock_pending_receipt(Succeeded);
        assert!(eval_receipt!(receipt.clone(), AcceptedOnL2, true).is_none());

        let receipt = mock_pending_receipt(Reverted { reason: Default::default() });
        assert!(eval_receipt!(receipt.clone(), false).unwrap().is_ok());

        let receipt = mock_pending_receipt(Reverted { reason: Default::default() });
        let evaluation = eval_receipt!(receipt.clone(), true).unwrap();
        assert_matches!(evaluation, Err(TransactionWaitingError::TransactionReverted(_)));
    }
}
