use std::collections::HashMap;
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
use torii_core::model::build_sql_query;
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::FELT_DELIMITER;
use torii_core::types::Entity;
use tracing::{error, trace};

use crate::proto;
use crate::proto::world::SubscribeEntityResponse;
use crate::server::map_row_to_entity;
use crate::types::{EntityKeysClause, PatternMatching};

pub(crate) const LOG_TARGET: &str = "torii::grpc::server::subscriptions::entity";

pub struct EntitiesSubscriber {
    /// Entity ids that the subscriber is interested in
    keys: Option<EntityKeysClause>,
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
        keys: Option<EntityKeysClause>,
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
    pool: Pool<Sqlite>,
    subs_manager: Arc<EntityManager>,
    model_cache: Arc<ModelCache>,
    simple_broker: Pin<Box<dyn Stream<Item = Entity> + Send>>,
}

impl Service {
    pub fn new(
        pool: Pool<Sqlite>,
        subs_manager: Arc<EntityManager>,
        model_cache: Arc<ModelCache>,
    ) -> Self {
        Self {
            pool,
            subs_manager,
            model_cache,
            simple_broker: Box::pin(SimpleBroker::<Entity>::subscribe()),
        }
    }

    async fn publish_updates(
        subs: Arc<EntityManager>,
        cache: Arc<ModelCache>,
        pool: Pool<Sqlite>,
        entity: &Entity,
    ) -> Result<(), Error> {
        let mut closed_stream = Vec::new();
        let hashed = FieldElement::from_str(&entity.id).map_err(ParseError::FromStr)?;
        let keys = entity
            .keys
            .trim_end_matches(FELT_DELIMITER)
            .split(FELT_DELIMITER)
            .map(FieldElement::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ParseError::FromStr)?;

        for (idx, sub) in subs.subscribers.read().await.iter() {
            // Check if the subscriber is interested in this entity
            // If we have a clause of hashed keys, then check that the id of the entity
            // is in the list of hashed keys.

            // If we have a clause of keys, then check that the key pattern of the entity
            // matches the key pattern of the subscriber.
            match &sub.keys {
                Some(EntityKeysClause::HashedKeys(hashed_keys)) => {
                    if !hashed_keys.contains(&hashed) {
                        continue;
                    }
                }
                Some(EntityKeysClause::Keys(clause)) => {
                    // if we have a model clause, then we need to check that the entity
                    // has an updated model and that the model name matches the clause
                    if let Some(updated_model) = &entity.updated_model {
                        if !clause.models.is_empty()
                            && !clause.models.contains(&updated_model.name())
                        {
                            continue;
                        }
                    }

                    // if the key pattern doesnt match our subscribers key pattern, skip
                    // ["", "0x0"] would match with keys ["0x...", "0x0", ...]
                    if clause.pattern_matching == PatternMatching::FixedLen
                        && keys.len() != clause.keys.len()
                    {
                        continue;
                    }

                    if !keys.iter().enumerate().all(|(idx, key)| {
                        // this is going to be None if our key pattern overflows the subscriber
                        // key pattern in this case we should skip
                        let sub_key = clause.keys.get(idx);

                        match sub_key {
                            Some(sub_key) => {
                                if sub_key == &FieldElement::ZERO {
                                    true
                                } else {
                                    key == sub_key
                                }
                            }
                            // we overflowed the subscriber key pattern
                            // but we're in VariableLen pattern matching
                            // so we should match all next keys
                            None => true,
                        }
                    }) {
                        continue;
                    }
                }
                // if None, then we are interested in all entities
                None => {}
            }

            if entity.updated_model.is_none() {
                let resp = proto::world::SubscribeEntityResponse {
                    entity: Some(proto::types::Entity {
                        hashed_keys: hashed.to_bytes_be().to_vec(),
                        models: vec![],
                    }),
                };

                if sub.sender.send(Ok(resp)).await.is_err() {
                    closed_stream.push(*idx);
                }

                continue;
            }

            let models_query = r#"
                    SELECT group_concat(entity_model.model_id) as model_ids
                    FROM entities
                    JOIN entity_model ON entities.id = entity_model.entity_id
                    WHERE entities.id = ?
                    GROUP BY entities.id
                "#;
            let (model_ids,): (String,) =
                sqlx::query_as(models_query).bind(&entity.id).fetch_one(&pool).await?;
            let model_ids: Vec<&str> = model_ids.split(',').collect();
            let schemas = cache.schemas(model_ids).await?;

            let (entity_query, arrays_queries) = build_sql_query(
                &schemas,
                "entities",
                "entity_id",
                Some("entities.id = ?"),
                Some("entities.id = ?"),
            )?;

            let row = sqlx::query(&entity_query).bind(&entity.id).fetch_one(&pool).await?;
            let mut arrays_rows = HashMap::new();
            for (name, query) in arrays_queries {
                let row = sqlx::query(&query).bind(&entity.id).fetch_all(&pool).await?;
                arrays_rows.insert(name, row);
            }

            let resp = proto::world::SubscribeEntityResponse {
                entity: Some(map_row_to_entity(&row, &arrays_rows, &schemas)?),
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
            let subs = Arc::clone(&pin.subs_manager);
            let cache = Arc::clone(&pin.model_cache);
            let pool = pin.pool.clone();
            tokio::spawn(async move {
                if let Err(e) = Service::publish_updates(subs, cache, pool, &entity).await {
                    error!(target = LOG_TARGET, error = %e, "Publishing entity update.");
                }
            });
        }

        Poll::Pending
    }
}
