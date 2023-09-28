//! Messaging service is a Future which is polling two streams,
//! one for gathering the messages from the settlement chain,
//! and an other one to settle messages on the settlement chain.
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::{Future, FutureExt, Stream, StreamExt};
use starknet::core::types::{FieldElement, MsgToL1};
use tokio::time::{interval_at, Instant, Interval};
use tracing::{error, info, trace};

use crate::backend::storage::transaction::{L1HandlerTransaction, Transaction};
use crate::backend::Backend;
use crate::messaging::starknet_messenger::HASH_EXEC;
use crate::messaging::{AnyMessenger, MessagingConfig, Messenger, MSGING_TARGET};
use crate::pool::TransactionPool;

pub struct MessageService {
    settler: MessageSettler,
    gatherer: MessageGatherer,
}

impl MessageService {
    /// Initializes a new instance from a configuration file's path.
    /// Will panic on failure to avoid continuing with invalid configuration.
    pub async fn new(
        config: MessagingConfig,
        backend: Arc<Backend>,
        transaction_pool: Arc<TransactionPool>,
    ) -> Self {
        let messenger = match AnyMessenger::from_config(config.clone()).await {
            Ok(m) => Arc::new(m),
            Err(e) => panic!(
                "Messaging could not be initialized: {:?}.\nVerify that the messaging target node \
                 (anvil or other katana) is running.\n",
                e
            ),
        };

        let gatherer = MessageGatherer::new(
            config.clone(),
            Arc::clone(&transaction_pool),
            Arc::clone(&messenger),
        );

        let settler = MessageSettler::new(config, Arc::clone(&backend), Arc::clone(&messenger));

        Self { settler, gatherer }
    }
}

impl Future for MessageService {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();

        // TODO: I'm a not sure if it's the right approach to have the two polled
        // continuously.
        while let Poll::Ready(Some(_)) = pin.settler.poll_next_unpin(cx) {}
        while let Poll::Ready(Some(_)) = pin.gatherer.poll_next_unpin(cx) {}

        Poll::Pending
    }
}

// Sync is not required, only readonly methods are called.
type ServiceFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// The gatherer is responsible of fetching messages,
/// to then add L1HandlerTx to the pool.
pub struct MessageGatherer {
    interval: Interval,
    transaction_pool: Arc<TransactionPool>,
    messenger: Arc<AnyMessenger>,
    gathering: Option<ServiceFuture<u64>>,
    settle_from_block: u64,
}

impl MessageGatherer {
    pub fn new(
        config: MessagingConfig,
        transaction_pool: Arc<TransactionPool>,
        messenger: Arc<AnyMessenger>,
    ) -> Self {
        let interval = interval_from_seconds(config.fetch_interval);

        Self {
            interval,
            transaction_pool: Arc::clone(&transaction_pool),
            messenger,
            gathering: None,
            settle_from_block: config.from_block,
        }
    }

    async fn gather_messages(
        messenger: Arc<AnyMessenger>,
        transaction_pool: Arc<TransactionPool>,
        from_block: u64,
    ) -> u64 {
        // 200 avoids any possible rejection from RPC with possibly lot's of messages.
        // TODO: May this be configurable?
        let max_block = 200;

        match messenger.gather_messages(from_block, max_block).await {
            Ok((last_block, l1_handler_txs)) => {
                for tx in &l1_handler_txs {
                    trace_l1_handler_tx_exec(tx);
                    transaction_pool.add_transaction(Transaction::L1Handler(tx.clone()));
                }
                last_block + 1
            }
            Err(e) => {
                error!("Error gathering messages: {:?}", e);
                // We stay at the same block to retry at next tick.
                from_block
            }
        }
    }
}

impl Stream for MessageGatherer {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        let interval = &mut pin.interval;

        if interval.poll_tick(cx).is_ready() && pin.gathering.is_none() {
            pin.gathering = Some(Box::pin(Self::gather_messages(
                pin.messenger.clone(),
                pin.transaction_pool.clone(),
                pin.settle_from_block,
            )));
        }

        // Poll the gathering future.
        if let Some(mut gathering) = pin.gathering.take() {
            if let Poll::Ready(last_block) = gathering.poll_unpin(cx) {
                pin.settle_from_block = last_block;
                return Poll::Ready(Some(()));
            } else {
                pin.gathering = Some(gathering)
            }
        }

        Poll::Pending
    }
}

/// The settler is responsible of sending messages,
/// to the settlement chain.
pub struct MessageSettler {
    interval: Interval,
    backend: Arc<Backend>,
    messenger: Arc<AnyMessenger>,
    settling: Option<ServiceFuture<()>>,
    local_from_block: u64,
}

impl MessageSettler {
    pub fn new(
        config: MessagingConfig,
        backend: Arc<Backend>,
        messenger: Arc<AnyMessenger>,
    ) -> Self {
        let interval = interval_from_seconds(config.fetch_interval);

        Self {
            interval,
            backend: Arc::clone(&backend),
            messenger,
            settling: None,
            // We always start settling messages from the block 0 of Katana
            // in the current implementation.
            // TODO: Think about katana state loading, may this be configurable then?
            local_from_block: 0,
        }
    }

    async fn settle_messages(messenger: Arc<AnyMessenger>, messages: Vec<MsgToL1>) {
        if let Ok(hashes) = messenger.settle_messages(&messages).await {
            trace_msg_to_l1_sent(&messages, &hashes);
        }
    }
}

impl Stream for MessageSettler {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        let interval = &mut pin.interval;

        if interval.poll_tick(cx).is_ready() && pin.settling.is_none() {
            let local_latest = pin.backend.blockchain.storage.read().latest_number;

            if pin.local_from_block > local_latest {
                return Poll::Ready(Some(()));
            }

            let mut messages = vec![];
            if let Some(block) =
                pin.backend.blockchain.storage.read().block_by_number(pin.local_from_block)
            {
                for o in &block.outputs {
                    messages.extend(o.messages_sent.clone());
                }
            }

            if !messages.is_empty() {
                pin.settling =
                    Some(Box::pin(Self::settle_messages(pin.messenger.clone(), messages.clone())));
            } else {
                trace!(target: MSGING_TARGET,
                       "Nothing messages for block: {:?}", pin.local_from_block);

                pin.local_from_block += 1;
                return Poll::Ready(Some(()));
            }
        }

        // Poll the settling future.
        if let Some(mut settling) = pin.settling.take() {
            if let Poll::Ready(()) = settling.poll_unpin(cx) {
                // +1 to move to the next local block to check messages to be
                // sent on the settlement chain.
                pin.local_from_block += 1;
                return Poll::Ready(Some(()));
            } else {
                pin.settling = Some(settling)
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
    let hash_exec_str = format!("{:#064x}", HASH_EXEC);

    for (i, m) in messages.iter().enumerate() {
        let payload_str: Vec<String> = m.payload.iter().map(|f| format!("{:#x}", *f)).collect();

        let hash = &hashes[i];

        if hash == &hash_exec_str {
            let to_address = &payload_str[0];
            let selector = &payload_str[1];
            let payload_str = &payload_str[2..];

            info!(target: MSGING_TARGET,
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

            info!(target: MSGING_TARGET,
                                          r"Message sent to settlement layer:
|     hash     | {}
| from_address | {:#x}
|  to_address  | {}
|   payload    | [{}]

",
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

    info!(
                        target: MSGING_TARGET,
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
