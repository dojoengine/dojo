use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::{Stream, StreamExt};
use rand::Rng;
use sqlx::{Pool, Sqlite};
use starknet::core::types::Felt;
use tokio::sync::mpsc::{
    channel, unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender,
};
use tokio::sync::RwLock;
use torii_sqlite::error::{Error, ParseError};
use torii_sqlite::simple_broker::SimpleBroker;
use torii_sqlite::types::ContractCursor as ContractUpdated;
use tracing::{error, trace};

use crate::proto;
use crate::proto::world::SubscribeIndexerResponse;

pub(crate) const LOG_TARGET: &str = "torii::grpc::server::subscriptions::indexer";

#[derive(Debug)]
pub struct IndexerSubscriber {
    /// Contract address that the subscriber is interested in
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
        pool: &Pool<Sqlite>,
        contract_address: Felt,
    ) -> Result<Receiver<Result<proto::world::SubscribeIndexerResponse, tonic::Status>>, Error>
    {
        let id = rand::thread_rng().gen::<usize>();
        let (sender, receiver) = channel(1);

        let mut statement = "SELECT * FROM contracts".to_string();

        let contracts: Vec<ContractUpdated> = if contract_address != Felt::ZERO {
            statement += " WHERE id = ?";

            sqlx::query_as(&statement)
                .bind(format!("{:#x}", contract_address))
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as(&statement).fetch_all(pool).await?
        };

        for contract in contracts {
            let _ = sender
                .send(Ok(SubscribeIndexerResponse {
                    head: contract.head,
                    tps: contract.tps,
                    last_block_timestamp: contract.last_block_timestamp,
                    contract_address: contract_address.to_bytes_be().to_vec(),
                }))
                .await;
        }
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
    simple_broker: Pin<Box<dyn Stream<Item = ContractUpdated> + Send>>,
    update_sender: UnboundedSender<ContractUpdated>,
}

impl Service {
    pub fn new(subs_manager: Arc<IndexerManager>) -> Self {
        let (update_sender, update_receiver) = unbounded_channel();
        let service = Self {
            simple_broker: Box::pin(SimpleBroker::<ContractUpdated>::subscribe()),
            update_sender,
        };

        tokio::spawn(Self::publish_updates(subs_manager, update_receiver));

        service
    }

    async fn publish_updates(
        subs: Arc<IndexerManager>,
        mut update_receiver: UnboundedReceiver<ContractUpdated>,
    ) {
        while let Some(update) = update_receiver.recv().await {
            if let Err(e) = Self::process_update(&subs, &update).await {
                error!(target = LOG_TARGET, error = %e, "Processing indexer update.");
            }
        }
    }

    async fn process_update(
        subs: &Arc<IndexerManager>,
        update: &ContractUpdated,
    ) -> Result<(), Error> {
        let mut closed_stream = Vec::new();
        let contract_address =
            Felt::from_str(&update.contract_address).map_err(ParseError::FromStr)?;

        for (idx, sub) in subs.subscribers.read().await.iter() {
            if sub.contract_address != Felt::ZERO && sub.contract_address != contract_address {
                continue;
            }

            let resp = SubscribeIndexerResponse {
                head: update.head,
                tps: update.tps,
                last_block_timestamp: update.last_block_timestamp,
                contract_address: contract_address.to_bytes_be().to_vec(),
            };

            if sub.sender.send(Ok(resp)).await.is_err() {
                closed_stream.push(*idx);
            }
        }

        for id in closed_stream {
            trace!(target = LOG_TARGET, id = %id, "Closing indexer updates stream.");
            subs.remove_subscriber(id).await
        }

        Ok(())
    }
}

impl Future for Service {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while let Poll::Ready(Some(update)) = this.simple_broker.poll_next_unpin(cx) {
            if let Err(e) = this.update_sender.send(update) {
                error!(target = LOG_TARGET, error = %e, "Sending indexer update to processor.");
            }
        }

        Poll::Pending
    }
}
