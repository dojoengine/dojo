use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::Stream;
use futures_util::StreamExt;
use rand::Rng;
use sqlx::{Pool, Sqlite};
use starknet::core::types::Felt;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use tokio::time::interval;
use torii_core::cache::ModelCache;
use torii_core::error::{Error, ParseError};
use torii_core::model::build_sql_query;
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::FELT_DELIMITER;
use torii_core::types::EventMessage;
use tracing::{error, trace};

use super::entity::EntitiesSubscriber;
use crate::proto;
use crate::proto::world::SubscribeEntityResponse;
use crate::server::map_row_to_entity;
use crate::types::{EntityKeysClause, PatternMatching};

pub(crate) const LOG_TARGET: &str = "torii::grpc::server::subscriptions::event_message";

#[derive(Debug, Default)]
pub struct EventMessageManager {
    subscribers: RwLock<HashMap<u64, EntitiesSubscriber>>,
}

impl EventMessageManager {
    pub async fn add_subscriber(
        &self,
        clauses: Vec<EntityKeysClause>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        let subscription_id = rand::thread_rng().gen::<u64>();
        let (sender, receiver) = channel(1);

        // NOTE: unlock issue with firefox/safari
        // initially send empty stream message to return from
        // initial subscribe call
        let _ = sender.send(Ok(SubscribeEntityResponse { entity: None, subscription_id })).await;

        self.subscribers
            .write()
            .await
            .insert(subscription_id, EntitiesSubscriber { clauses, sender });

        Ok(receiver)
    }

    pub async fn update_subscriber(&self, id: u64, clauses: Vec<EntityKeysClause>) {
        let sender = {
            let subscribers = self.subscribers.read().await;
            if let Some(subscriber) = subscribers.get(&id) {
                subscriber.sender.clone()
            } else {
                return; // Subscriber not found, exit early
            }
        };

        self.subscribers.write().await.insert(id, EntitiesSubscriber { clauses, sender });
    }

    pub(super) async fn remove_subscriber(&self, id: u64) {
        self.subscribers.write().await.remove(&id);
    }
}

#[must_use = "Service does nothing unless polled"]
#[allow(missing_debug_implementations)]
pub struct Service {
    simple_broker: Pin<Box<dyn Stream<Item = EventMessage> + Send>>,
    update_sender: Sender<EventMessage>,
}

impl Service {
    pub fn new(
        pool: Pool<Sqlite>,
        subs_manager: Arc<EventMessageManager>,
        model_cache: Arc<ModelCache>,
    ) -> Self {
        let (update_sender, update_receiver) = channel(100);
        let service = Self {
            simple_broker: Box::pin(SimpleBroker::<EventMessage>::subscribe()),
            update_sender,
        };

        // Spawn a task to process event message updates
        tokio::spawn(Self::process_updates(
            Arc::clone(&subs_manager),
            Arc::clone(&model_cache),
            pool,
            update_receiver,
        ));

        service
    }

    async fn process_updates(
        subs: Arc<EventMessageManager>,
        cache: Arc<ModelCache>,
        pool: Pool<Sqlite>,
        mut update_receiver: Receiver<EventMessage>,
    ) {
        let mut interval = interval(Duration::from_millis(100));
        let mut pending_updates = Vec::new();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if !pending_updates.is_empty() {
                        for event_message in pending_updates.drain(..) {
                            if let Err(e) = Self::publish_updates(Arc::clone(&subs), Arc::clone(&cache), pool.clone(), &event_message).await {
                                error!(target = LOG_TARGET, error = %e, "Publishing event message update.");
                            }
                        }
                    }
                }
                Some(event_message) = update_receiver.recv() => {
                    pending_updates.push(event_message);
                }
            }
        }
    }

    async fn publish_updates(
        subs: Arc<EventMessageManager>,
        cache: Arc<ModelCache>,
        pool: Pool<Sqlite>,
        entity: &EventMessage,
    ) -> Result<(), Error> {
        let mut closed_stream = Vec::new();
        let hashed = Felt::from_str(&entity.id).map_err(ParseError::FromStr)?;
        let keys = entity
            .keys
            .trim_end_matches(FELT_DELIMITER)
            .split(FELT_DELIMITER)
            .map(Felt::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ParseError::FromStr)?;

        for (idx, sub) in subs.subscribers.read().await.iter() {
            // Check if the subscriber is interested in this entity
            // If we have a clause of hashed keys, then check that the id of the entity
            // is in the list of hashed keys.

            // If we have a clause of keys, then check that the key pattern of the entity
            // matches the key pattern of the subscriber.
            if !sub.clauses.is_empty()
                && !sub.clauses.iter().any(|clause| match clause {
                    EntityKeysClause::HashedKeys(hashed_keys) => {
                        hashed_keys.is_empty() || hashed_keys.contains(&hashed)
                    }
                    EntityKeysClause::Keys(clause) => {
                        // if we have a model clause, then we need to check that the entity
                        // has an updated model and that the model name matches the clause
                        if let Some(updated_model) = &entity.updated_model {
                            let name = updated_model.name();
                            let (namespace, name) = name.split_once('-').unwrap();

                            if !clause.models.is_empty()
                                && !clause.models.iter().any(|clause_model| {
                                    let (clause_namespace, clause_model) =
                                        clause_model.split_once('-').unwrap();
                                    // if both namespace and model are empty, we should match all.
                                    // if namespace is specified and model is empty or * we should
                                    // match all models in the
                                    // namespace if namespace
                                    // and model are specified, we should match the
                                    // specific model
                                    (clause_namespace.is_empty()
                                        || clause_namespace == namespace
                                        || clause_namespace == "*")
                                        && (clause_model.is_empty()
                                            || clause_model == name
                                            || clause_model == "*")
                                })
                            {
                                return false;
                            }
                        }

                        // if the key pattern doesnt match our subscribers key pattern, skip
                        // ["", "0x0"] would match with keys ["0x...", "0x0", ...]
                        if clause.pattern_matching == PatternMatching::FixedLen
                            && keys.len() != clause.keys.len()
                        {
                            return false;
                        }

                        return keys.iter().enumerate().all(|(idx, key)| {
                            // this is going to be None if our key pattern overflows the subscriber
                            // key pattern in this case we should skip
                            let sub_key = clause.keys.get(idx);

                            match sub_key {
                                // the key in the subscriber must match the key of the entity
                                // athis index
                                Some(Some(sub_key)) => key == sub_key,
                                // otherwise, if we have no key we should automatically match.
                                // or.. we overflowed the subscriber key pattern
                                // but we're in VariableLen pattern matching
                                // so we should match all next keys
                                _ => true,
                            }
                        });
                    }
                })
            {
                continue;
            }

            // publish all updates if ids is empty or only ids that are subscribed to
            let models_query = r#"
                    SELECT group_concat(event_model.model_id) as model_ids
                    FROM event_messages
                    JOIN event_model ON event_messages.id = event_model.entity_id
                    WHERE event_messages.id = ?
                    GROUP BY event_messages.id
                "#;
            let (model_ids,): (String,) =
                sqlx::query_as(models_query).bind(&entity.id).fetch_one(&pool).await?;
            let model_ids: Vec<Felt> = model_ids
                .split(',')
                .map(Felt::from_str)
                .collect::<Result<_, _>>()
                .map_err(ParseError::FromStr)?;
            let schemas = cache.models(&model_ids).await?.into_iter().map(|m| m.schema).collect();

            let (entity_query, arrays_queries, _) = build_sql_query(
                &schemas,
                "event_messages",
                "event_message_id",
                Some("event_messages.id = ?"),
                Some("event_messages.id = ?"),
                None,
                None,
            )?;

            let row = sqlx::query(&entity_query).bind(&entity.id).fetch_one(&pool).await?;
            let mut arrays_rows = HashMap::new();
            for (name, query) in arrays_queries {
                let rows = sqlx::query(&query).bind(&entity.id).fetch_all(&pool).await?;
                arrays_rows.insert(name, rows);
            }

            let resp = proto::world::SubscribeEntityResponse {
                entity: Some(map_row_to_entity(&row, &arrays_rows, schemas.clone())?),
                subscription_id: *idx,
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

        while let Poll::Ready(Some(event_message)) = pin.simple_broker.poll_next_unpin(cx) {
            let sender = pin.update_sender.clone();
            tokio::spawn(async move {
                if let Err(e) = sender.send(event_message).await {
                    error!(target = LOG_TARGET, error = %e, "Sending event message update to channel.");
                }
            });
        }

        Poll::Pending
    }
}
