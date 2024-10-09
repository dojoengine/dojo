use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Stream;
use futures_util::StreamExt;
use rand::Rng;
use starknet::core::types::Felt;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use torii_core::error::{Error, ParseError};
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::FELT_DELIMITER;
use torii_core::types::OptimisticEntity;
use tracing::{error, trace};

use crate::proto;
use crate::proto::world::SubscribeEntityResponse;
use crate::types::{EntityKeysClause, PatternMatching};

pub(crate) const LOG_TARGET: &str = "torii::grpc::server::subscriptions::entity";

#[derive(Debug)]
pub struct EntitiesSubscriber {
    /// Entity ids that the subscriber is interested in
    pub(crate) clauses: Vec<EntityKeysClause>,
    /// The channel to send the response back to the subscriber.
    pub(crate) sender: Sender<Result<proto::world::SubscribeEntityResponse, tonic::Status>>,
}
#[derive(Debug, Default)]
pub struct EntityManager {
    subscribers: RwLock<HashMap<u64, EntitiesSubscriber>>,
}

impl EntityManager {
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
    subs_manager: Arc<EntityManager>,
    simple_broker: Pin<Box<dyn Stream<Item = OptimisticEntity> + Send>>,
}

impl Service {
    pub fn new(subs_manager: Arc<EntityManager>) -> Self {
        Self {
            subs_manager,
            simple_broker: Box::pin(SimpleBroker::<OptimisticEntity>::subscribe()),
        }
    }

    async fn publish_updates(
        subs: Arc<EntityManager>,
        entity: &OptimisticEntity,
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

            if entity.deleted {
                let resp = proto::world::SubscribeEntityResponse {
                    entity: Some(proto::types::Entity {
                        hashed_keys: hashed.to_bytes_be().to_vec(),
                        models: vec![],
                    }),
                    subscription_id: *idx,
                };

                if sub.sender.send(Ok(resp)).await.is_err() {
                    closed_stream.push(*idx);
                }

                continue;
            }

            // This should NEVER be None
            let model = entity.updated_model.as_ref().unwrap().as_struct().unwrap().clone();
            let resp = proto::world::SubscribeEntityResponse {
                entity: Some(proto::types::Entity {
                    hashed_keys: hashed.to_bytes_be().to_vec(),
                    models: vec![model.into()],
                }),
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

        while let Poll::Ready(Some(entity)) = pin.simple_broker.poll_next_unpin(cx) {
            let subs = Arc::clone(&pin.subs_manager);
            tokio::spawn(async move {
                if let Err(e) = Service::publish_updates(subs, &entity).await {
                    error!(target = LOG_TARGET, error = %e, "Publishing entity update.");
                }
            });
        }

        Poll::Pending
    }
}
