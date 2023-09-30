use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use ::starknet::core::types::{FieldElement, MsgToL1};
use futures::{Future, FutureExt, Stream};
use tokio::time::{interval_at, Instant, Interval};
use tracing::{error, info};

use super::{MessagingConfig, Messenger, MessengerMode, MessengerResult, LOG_TARGET};
use crate::backend::storage::transaction::{L1HandlerTransaction, Transaction};
use crate::backend::Backend;
use crate::pool::TransactionPool;

type MessagingFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;
type MessageGatheringFuture = MessagingFuture<MessengerResult<(u64, usize)>>;
type MessageSettlingFuture = MessagingFuture<MessengerResult<Option<(u64, usize)>>>;

pub struct MessagingService {
    /// The interval at which the service will perform the messaging operations.
    interval: Interval,
    backend: Arc<Backend>,
    pool: Arc<TransactionPool>,
    /// The messenger mode the service is running in.
    messenger: Arc<MessengerMode>,
    /// The block number of the settlement chain from which messages will be gathered.
    gather_from_block: u64,
    /// The message gathering future.
    msg_gather_fut: Option<MessageGatheringFuture>,
    /// The block number of the local blockchain from which messages will be settled.
    settle_from_block: u64,
    /// The message settling future.
    msg_settle_fut: Option<MessageSettlingFuture>,
}

impl MessagingService {
    /// Initializes a new instance from a configuration file's path.
    /// Will panic on failure to avoid continuing with invalid configuration.
    pub async fn new(
        config: MessagingConfig,
        pool: Arc<TransactionPool>,
        backend: Arc<Backend>,
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

        Ok(Self {
            pool,
            backend,
            interval,
            messenger,
            gather_from_block,
            settle_from_block: 0,
            msg_gather_fut: None,
            msg_settle_fut: None,
        })
    }

    async fn gather_messages(
        messenger: Arc<MessengerMode>,
        pool: Arc<TransactionPool>,
        from_block: u64,
    ) -> MessengerResult<(u64, usize)> {
        // 200 avoids any possible rejection from RPC with possibly lot's of messages.
        // TODO: May this be configurable?
        let max_block = 200;

        match messenger.as_ref() {
            MessengerMode::Ethereum(inner) => {
                let (block_num, txs) = inner.gather_messages(from_block, max_block).await?;
                let txs_count = txs.len();

                txs.into_iter().for_each(|tx| {
                    trace_l1_handler_tx_exec(&tx);
                    pool.add_transaction(Transaction::L1Handler(tx))
                });

                Ok((block_num, txs_count))
            }

            MessengerMode::Starknet(inner) => {
                let (block_num, txs) = inner.gather_messages(from_block, max_block).await?;
                let txs_count = txs.len();

                txs.into_iter().for_each(|tx| {
                    trace_l1_handler_tx_exec(&tx);
                    pool.add_transaction(Transaction::L1Handler(tx))
                });

                Ok((block_num, txs_count))
            }
        }
    }

    async fn settle_messages(
        block_num: u64,
        backend: Arc<Backend>,
        messenger: Arc<MessengerMode>,
    ) -> MessengerResult<Option<(u64, usize)>> {
        let Some(messages) = backend
            .blockchain
            .storage
            .read()
            .block_by_number(block_num)
            .map(|block| &block.outputs)
            .map(|outputs| {
                outputs.iter().flat_map(|o| o.messages_sent.clone()).collect::<Vec<MsgToL1>>()
            })
        else {
            return Ok(None);
        };

        if messages.is_empty() {
            Ok(Some((block_num, 0)))
        } else {
            match messenger.as_ref() {
                MessengerMode::Ethereum(inner) => {
                    let hashes = inner
                        .settle_messages(&messages)
                        .await
                        .map(|hashes| hashes.iter().map(|h| format!("{h:#x}")).collect())?;
                    trace_msg_to_l1_sent(&messages, &hashes);
                    Ok(Some((block_num, hashes.len())))
                }

                MessengerMode::Starknet(inner) => {
                    let hashes = inner
                        .settle_messages(&messages)
                        .await
                        .map(|hashes| hashes.iter().map(|h| format!("{h:#x}")).collect())?;
                    trace_msg_to_l1_sent(&messages, &hashes);
                    Ok(Some((block_num, hashes.len())))
                }
            }
        }
    }
}

pub enum MessagingOutcome {
    Gather {
        /// The latest block number of the settlement chain from which messages were gathered.
        lastest_block: u64,
        /// The number of settlement chain messages gathered up until `latest_block`.
        msg_count: usize,
    },
    Settle {
        /// The current local block number from which messages were settled.
        block_num: u64,
        /// The number of messages settled on `block_num`.
        msg_count: usize,
    },
}

impl Stream for MessagingService {
    type Item = MessagingOutcome;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        if pin.interval.poll_tick(cx).is_ready() {
            if pin.msg_gather_fut.is_none() {
                pin.msg_gather_fut = Some(Box::pin(Self::gather_messages(
                    pin.messenger.clone(),
                    pin.pool.clone(),
                    pin.gather_from_block,
                )));
            }

            if pin.msg_settle_fut.is_none() {
                let local_latest_block_num = pin.backend.blockchain.storage.read().latest_number;
                if pin.settle_from_block <= local_latest_block_num {
                    pin.msg_settle_fut = Some(Box::pin(Self::settle_messages(
                        pin.settle_from_block,
                        pin.backend.clone(),
                        pin.messenger.clone(),
                    )))
                }
            }
        }

        // Poll the gathering future.
        if let Some(mut gather_fut) = pin.msg_gather_fut.take() {
            match gather_fut.poll_unpin(cx) {
                Poll::Ready(Ok((last_block, msg_count))) => {
                    pin.gather_from_block = last_block + 1;
                    return Poll::Ready(Some(MessagingOutcome::Gather {
                        lastest_block: last_block,
                        msg_count,
                    }));
                }
                Poll::Ready(Err(e)) => {
                    error!(target: LOG_TARGET, "error gathering messages for block {}: {e}", pin.gather_from_block);
                    return Poll::Pending;
                }
                Poll::Pending => pin.msg_gather_fut = Some(gather_fut),
            }
        }

        // Poll the settling future.
        if let Some(mut settle_fut) = pin.msg_settle_fut.take() {
            match settle_fut.poll_unpin(cx) {
                Poll::Ready(Ok(Some((block_num, msg_count)))) => {
                    // +1 to move to the next local block to check messages to be
                    // sent on the settlement chain.
                    pin.settle_from_block += 1;
                    return Poll::Ready(Some(MessagingOutcome::Settle { block_num, msg_count }));
                }
                Poll::Ready(Err(e)) => {
                    error!(target: LOG_TARGET, "error settling messages for block {}: {e}", pin.settle_from_block);
                    return Poll::Pending;
                }
                Poll::Ready(_) => return Poll::Pending,
                Poll::Pending => pin.msg_settle_fut = Some(settle_fut),
            }
        }

        Poll::Pending
    }
}

/// Returns an `Interval` from the given seconds.
fn interval_from_seconds(secs: u64) -> Interval {
    let duration = Duration::from_secs(secs);
    let mut interval = interval_at(Instant::now() + duration, duration);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval
}

fn trace_msg_to_l1_sent(messages: &Vec<MsgToL1>, hashes: &Vec<String>) {
    assert_eq!(messages.len(), hashes.len());
    let hash_exec_str = format!("{:#064x}", super::starknet::HASH_EXEC);

    for (i, m) in messages.iter().enumerate() {
        let payload_str: Vec<String> = m.payload.iter().map(|f| format!("{:#x}", *f)).collect();

        let hash = &hashes[i];

        if hash == &hash_exec_str {
            let to_address = &payload_str[0];
            let selector = &payload_str[1];
            let payload_str = &payload_str[2..];

            #[rustfmt::skip]
            info!(
                target: LOG_TARGET,
                r"Message executed on settlement layer:
| from_address | {:#x}
|  to_address  | {}
|   selector   | {}
|   payload    | [{}]

",
                m.from_address,
                to_address,
                selector,
                payload_str.join(", ")
            );
        } else {
            // We check for magic value 'MSG' used only when we are doing L3-L2 messaging.
            let (to_address, payload_str) = if format!("{:#x}", m.to_address) == "0x4d5347" {
                (payload_str[0].clone(), &payload_str[1..])
            } else {
                (format!("{:#x}", m.to_address), &payload_str[..])
            };

            #[rustfmt::skip]
            info!(
                target: LOG_TARGET,
                r#"Message sent to settlement layer:
|     hash     | {}
| from_address | {:#x}
|  to_address  | {}
|   payload    | [{}]

"#,
                hash.as_str(),
                m.from_address,
                to_address,
                payload_str.join(", ")
            );
        }
    }
}

fn trace_l1_handler_tx_exec(tx: &L1HandlerTransaction) {
    let calldata_str: Vec<String> =
        tx.inner.calldata.0.iter().map(|f| format!("{:#x}", FieldElement::from(*f))).collect();

    #[rustfmt::skip]
    info!(
        target: LOG_TARGET,
        r"L1Handler transaction added to the pool:
|      tx_hash     | {:#x}
| contract_address | {:#x}
|     selector     | {:#x}
|     calldata     | [{}]

",
        FieldElement::from(tx.inner.transaction_hash.0),
        FieldElement::from(*tx.inner.contract_address.0.key()),
        FieldElement::from(tx.inner.entry_point_selector.0),
        calldata_str.join(", ")
    );
}
