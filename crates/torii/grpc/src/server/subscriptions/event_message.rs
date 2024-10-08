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
use tokio::sync::mpsc::{channel, Receiver, unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;
use torii_core::error::{Error, ParseError};
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::FELT_DELIMITER;
use torii_core::types::OptimisticEventMessage;
use tracing::{error, trace};

use super::entity::EntitiesSubscriber;
use crate::proto;
use crate::proto::world::SubscribeEntityResponse;
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
    simple_broker: Pin<Box<dyn Stream<Item = OptimisticEventMessage> + Send>>,
    event_sender: UnboundedSender<OptimisticEventMessage>,
}

impl Service {
    pub fn new(subs_manager: Arc<EventMessageManager>) -> Self {
        let (event_sender, event_receiver) = unbounded_channel();
        let service = Self {
            simple_broker: Box::pin(SimpleBroker::<OptimisticEventMessage>::subscribe()),
            event_sender,
        };

        tokio::spawn(Self::publish_updates(subs_manager, event_receiver));

        service
    }

    async fn publish_updates(
        subs: Arc<EventMessageManager>,
        mut event_receiver: UnboundedReceiver<OptimisticEventMessage>,
    ) {
        while let Some(event) = event_receiver.recv().await {
            if let Err(e) = Self::process_event_update(&subs, &event).await {
                error!(target = LOG_TARGET, error = %e, "Processing event update.");
            }
        }
    }

    async fn process_event_update(
        subs: &Arc<EventMessageManager>,
        entity: &OptimisticEventMessage,
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

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        while let Poll::Ready(Some(event)) = this.simple_broker.poll_next_unpin(cx) {
            if let Err(e) = this.event_sender.send(event) {
                error!(target = LOG_TARGET, error = %e, "Sending event update to processor.");
            }
        }

        Poll::Pending
    }
}
