use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::{Future, FutureExt, Stream};
use katana_chain_spec::ChainSpec;
use katana_pool::{TransactionPool, TxPool};
use katana_primitives::chain::ChainId;
use katana_primitives::transaction::{ExecutableTxWithHash, L1HandlerTx, TxHash};
use tokio::time::{interval_at, Instant, Interval};
use tracing::{error, info, warn};

use super::{MessagingConfig, Messenger, MessengerMode, MessengerResult, LOG_TARGET};

type MessagingFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type MessageGatheringFuture = MessagingFuture<MessengerResult<(u64, usize)>>;

#[allow(missing_debug_implementations)]
pub struct MessagingService {
    /// The interval at which the service will perform the messaging operations.
    interval: Interval,
    chain_spec: Arc<ChainSpec>,
    pool: TxPool,
    /// The messenger mode the service is running in.
    messenger: Arc<MessengerMode>,
    /// The block number of the settlement chain from which messages will be gathered.
    gather_from_block: u64,
    /// The message gathering future.
    msg_gather_fut: Option<MessageGatheringFuture>,
}

impl MessagingService {
    /// Initializes a new instance from a configuration file's path.
    /// Will panic on failure to avoid continuing with invalid configuration.
    pub async fn new(
        config: MessagingConfig,
        chain_spec: Arc<ChainSpec>,
        pool: TxPool,
    ) -> anyhow::Result<Self> {
        let gather_from_block = config.from_block;
        let interval = interval_from_seconds(config.interval);
        let messenger = match MessengerMode::from_config(config).await {
            Ok(m) => Arc::new(m),
            Err(_) => {
                panic!(
                    "Messaging could not be initialized.\nVerify that the messaging target node \
                     (anvil or other katana) is running.\n",
                )
            }
        };

        Ok(Self { pool, interval, messenger, chain_spec, gather_from_block, msg_gather_fut: None })
    }

    async fn gather_messages(
        messenger: Arc<MessengerMode>,
        pool: TxPool,
        chain_id: ChainId,
        from_block: u64,
    ) -> MessengerResult<(u64, usize)> {
        // 200 avoids any possible rejection from RPC with possibly lot's of messages.
        // TODO: May this be configurable?
        let max_block = 200;

        match messenger.as_ref() {
            MessengerMode::Ethereum(inner) => {
                let (block_num, txs) =
                    inner.gather_messages(from_block, max_block, chain_id).await?;
                let txs_count = txs.len();

                txs.into_iter().for_each(|tx| {
                    let hash = tx.calculate_hash();
                    trace_l1_handler_tx_exec(hash, &tx);

                    // ignore result because L1Handler tx will always be valid
                    let _ =
                        pool.add_transaction(ExecutableTxWithHash { hash, transaction: tx.into() });
                });

                Ok((block_num, txs_count))
            }

            MessengerMode::Starknet(inner) => {
                let (block_num, txs) =
                    inner.gather_messages(from_block, max_block, chain_id).await?;
                let txs_count = txs.len();

                txs.into_iter().for_each(|tx| {
                    let hash = tx.calculate_hash();
                    trace_l1_handler_tx_exec(hash, &tx);

                    // ignore result because L1Handler tx will always be valid
                    let tx = ExecutableTxWithHash { hash, transaction: tx.into() };
                    let _ = pool.add_transaction(tx);
                });

                Ok((block_num, txs_count))
            }

            MessengerMode::Sovereign(_) => Ok((0, 0)),
        }
    }
}

#[derive(Debug)]
pub struct MessagingOutcome {
    /// The latest block number of the settlement chain from which messages were gathered.
    pub lastest_block: u64,
    /// The number of settlement chain messages gathered up until `latest_block`.
    pub msg_count: usize,
}

impl Stream for MessagingService {
    type Item = MessagingOutcome;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if pin.interval.poll_tick(cx).is_ready() && pin.msg_gather_fut.is_none() {
            pin.msg_gather_fut = Some(Box::pin(Self::gather_messages(
                pin.messenger.clone(),
                pin.pool.clone(),
                pin.chain_spec.id(),
                pin.gather_from_block,
            )));
        }

        // Poll the gathering future.
        if let Some(mut gather_fut) = pin.msg_gather_fut.take() {
            match gather_fut.poll_unpin(cx) {
                Poll::Ready(Ok((last_block, msg_count))) => {
                    pin.gather_from_block = last_block + 1;
                    return Poll::Ready(Some(MessagingOutcome {
                        lastest_block: last_block,
                        msg_count,
                    }));
                }
                Poll::Ready(Err(e)) => {
                    error!(
                        target: LOG_TARGET,
                        block = %pin.gather_from_block,
                        error = %e,
                        "Gathering messages for block."
                    );
                    return Poll::Pending;
                }
                Poll::Pending => pin.msg_gather_fut = Some(gather_fut),
            }
        }

        Poll::Pending
    }
}

/// Returns an `Interval` from the given seconds.
fn interval_from_seconds(secs: u64) -> Interval {
    let secs = if secs == 0 {
        warn!(target: LOG_TARGET, "Messaging interval is 0, using 1 second instead.");
        1
    } else {
        secs
    };

    let duration = Duration::from_secs(secs);
    let mut interval = interval_at(Instant::now() + duration, duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval
}

fn trace_l1_handler_tx_exec(hash: TxHash, tx: &L1HandlerTx) {
    let calldata_str: Vec<_> = tx.calldata.iter().map(|f| format!("{f:#x}")).collect();

    #[rustfmt::skip]
    info!(
        target: LOG_TARGET,
        tx_hash = %format!("{:#x}", hash),
        contract_address = %tx.contract_address,
        selector = %format!("{:#x}", tx.entry_point_selector),
        calldata = %calldata_str.join(", "),
        "L1Handler transaction added to the pool.",
    );
}
