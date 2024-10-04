use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::{Stream, StreamExt};
use rand::Rng;
use starknet::core::types::Felt;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use torii_core::error::Error;
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Contract as ContractUpdated;
use tracing::{error, trace};

use crate::proto;
use crate::proto::world::SubscribeIndexerResponse;

pub(crate) const LOG_TARGET: &str = "torii::grpc::server::subscriptions::event";

#[derive(Debug)]
pub struct IndexerSubscriber {
    /// Event keys that the subscriber is interested in
    contract_address: Felt,
    /// The channel to send the response back to the subscriber.
    sender: Sender<Result<proto::world::SubscribeIndexerResponse, tonic::Status>>,
}

#[derive(Debug, Default)]
pub struct IndexerManager {
    subscribers: RwLock<HashMap<usize, IndexerSubscriber>>,
}

impl IndexerManager {
    pub async fn add_subscriber(
        &self,
        contract_address: Felt,
    ) -> Result<Receiver<Result<proto::world::SubscribeIndexerResponse, tonic::Status>>, Error>
    {
        let id = rand::thread_rng().gen::<usize>();
        let (sender, receiver) = channel(1);

        // NOTE: unlock issue with firefox/safari
        // initially send empty stream message to return from
        // initial subscribe call
        let _ = sender
            .send(Ok(SubscribeIndexerResponse {
                head: 0,
                tps: 0,
                last_block_timestamp: 0,
                contract_address: contract_address.to_bytes_be().to_vec(),
            }))
            .await;

        self.subscribers.write().await.insert(id, IndexerSubscriber { contract_address, sender });

        Ok(receiver)
    }

    pub(super) async fn remove_subscriber(&self, id: usize) {
        self.subscribers.write().await.remove(&id);
    }
}

#[must_use = "Service does nothing unless polled"]
#[allow(missing_debug_implementations)]
pub struct Service {
    subs_manager: Arc<IndexerManager>,
    simple_broker: Pin<Box<dyn Stream<Item = ContractUpdated> + Send>>,
}

impl Service {
    pub fn new(subs_manager: Arc<IndexerManager>) -> Self {
        Self { subs_manager, simple_broker: Box::pin(SimpleBroker::<ContractUpdated>::subscribe()) }
    }

    async fn publish_updates(
        subs: Arc<IndexerManager>,
        update: &ContractUpdated,
    ) -> Result<(), Error> {
        let mut closed_stream = Vec::new();

        for (idx, sub) in subs.subscribers.read().await.iter() {
            if sub.contract_address != Felt::ZERO && sub.contract_address != update.contract_address
            {
                continue;
            }

            let resp = SubscribeIndexerResponse {
                head: update.head,
                tps: update.tps,
                last_block_timestamp: update.last_block_timestamp,
                contract_address: update.contract_address.to_bytes_be().to_vec(),
            };

            if sub.sender.send(Ok(resp)).await.is_err() {
                closed_stream.push(*idx);
            }
        }

        for id in closed_stream {
            trace!(target = LOG_TARGET, id = %id, "Closing events stream.");
            subs.remove_subscriber(id).await
        }

        Ok(())
    }
}

impl Future for Service {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let pin = self.get_mut();

        while let Poll::Ready(Some(event)) = pin.simple_broker.poll_next_unpin(cx) {
            let subs = Arc::clone(&pin.subs_manager);
            tokio::spawn(async move {
                if let Err(e) = Service::publish_updates(subs, &event).await {
                    error!(target = LOG_TARGET, error = %e, "Publishing indexer update.");
                }
            });
        }

        Poll::Pending
    }
}
