use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Stream;
use futures_util::StreamExt;
use rand::Rng;
use sqlx::{Pool, Sqlite};
use starknet_crypto::FieldElement;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use torii_core::cache::ModelCache;
use torii_core::error::{Error, ParseError};
use torii_core::model::{build_sql_query, map_row_to_ty};
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::FELT_DELIMITER;
use torii_core::types::Entity;
use tracing::{error, trace};

use crate::proto;
use crate::proto::world::SubscribeEntityResponse;
use crate::server::{DojoWorld, ENTITIES_ENTITY_RELATION_COLUMN, ENTITIES_MODEL_RELATION_TABLE, ENTITIES_TABLE};

pub(crate) const LOG_TARGET: &str = "torii::grpc::server::subscriptions::entity";

pub struct EntitiesSubscriber {
    /// Entity ids that the subscriber is interested in
    keys: Option<proto::types::EntityKeysClause>,
    /// The channel to send the response back to the subscriber.
    sender: Sender<Result<proto::world::SubscribeEntityResponse, tonic::Status>>,
}

#[derive(Default)]
pub struct EntityManager {
    subscribers: RwLock<HashMap<usize, EntitiesSubscriber>>,
}

impl EntityManager {
    pub async fn add_subscriber(
        &self,
        keys: Option<proto::types::EntityKeysClause>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        let id = rand::thread_rng().gen::<usize>();
        let (sender, receiver) = channel(1);

        // NOTE: unlock issue with firefox/safari
        // initially send empty stream message to return from
        // initial subscribe call
        let _ = sender.send(Ok(SubscribeEntityResponse { entity: None })).await;

        self.subscribers.write().await.insert(id, EntitiesSubscriber { keys, sender });

        Ok(receiver)
    }

    pub(super) async fn remove_subscriber(&self, id: usize) {
        self.subscribers.write().await.remove(&id);
    }
}

#[must_use = "Service does nothing unless polled"]
pub struct Service {
    world: Arc<DojoWorld>,
    simple_broker: Pin<Box<dyn Stream<Item = Entity> + Send>>,
}

impl Service {
    pub fn new(
        world: Arc<DojoWorld>,
    ) -> Self {
        Self {
            world,
            simple_broker: Box::pin(SimpleBroker::<Entity>::subscribe()),
        }
    }

    async fn publish_updates(
        world: &DojoWorld,
        entity: &Entity,
    ) -> Result<(), Error> {
        let mut closed_stream = Vec::new();
        let keys = entity
            .keys
            .trim_end_matches(FELT_DELIMITER)
            .split(FELT_DELIMITER)
            .map(FieldElement::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ParseError::FromStr)?;

        for (idx, sub) in world.entity_manager.subscribers.read().await.iter() {
            let entities = match sub.keys {
                Some(proto::types::EntityKeysClause{clause_type: Some(proto::types::entity_keys_clause::ClauseType::HashedKeys(clause))}) => world.query_by_hashed_keys(ENTITIES_TABLE, ENTITIES_MODEL_RELATION_TABLE, ENTITIES_ENTITY_RELATION_COLUMN, Some(clause), None, None).await?,
                Some(proto::types::EntityKeysClause{clause_type: Some(proto::types::entity_keys_clause::ClauseType::Keys(clause))}) => world.query_by_keys(ENTITIES_TABLE, ENTITIES_MODEL_RELATION_TABLE, ENTITIES_ENTITY_RELATION_COLUMN, clause, None, None).await?,
                Some(proto::types::EntityKeysClause{clause_type: None}) => world.query_by_hashed_keys(ENTITIES_TABLE, ENTITIES_MODEL_RELATION_TABLE, ENTITIES_ENTITY_RELATION_COLUMN, None, None, None).await?,
                None => world.query_by_hashed_keys(ENTITIES_TABLE, ENTITIES_MODEL_RELATION_TABLE, ENTITIES_ENTITY_RELATION_COLUMN, None, None, None).await?,
            };

            let resp = proto::world::SubscribeEntityResponse {
                entity: Some(proto::types::Entity {
                    hashed_keys: hashed.to_bytes_be().to_vec(),
                    models,
                }),
            };

            if sub.sender.send(Ok(resp)).await.is_err() {
                closed_stream.push(*idx);
            }
        }

        for id in closed_stream {
            trace!(target = LOG_TARGET, id = %id, "Closing entity stream.");
            subs.remove_subscriber(id).await
        }

        Ok(())
    }
}

impl Future for Service {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        let pin = self.get_mut();

        while let Poll::Ready(Some(entity)) = pin.simple_broker.poll_next_unpin(cx) {
            tokio::spawn(async move {
                if let Err(e) = Service::publish_updates(&self.world, &entity).await {
                    error!(target = LOG_TARGET, error = %e, "Publishing entity update.");
                }
            });
        }

        Poll::Pending
    }
}
