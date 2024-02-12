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
use torii_core::types::Message;
use tracing::{error, trace};

use crate::proto;

pub struct MessagesSubscriber {
    /// Topic that the subscriber is interested in
    topic: String,
    /// The channel to send the response back to the subscriber.
    sender: Sender<Result<proto::world::SubscribeMessagesResponse, tonic::Status>>,
}

#[derive(Default)]
pub struct MessageManager {
    subscribers: RwLock<HashMap<usize, MessagesSubscriber>>,
}

impl MessageManager {
    pub async fn add_subscriber(
        &self,
        topic: String,
    ) -> Result<Receiver<Result<proto::world::SubscribeMessagesResponse, tonic::Status>>, Error>
    {
        let id = rand::thread_rng().gen::<usize>();
        let (sender, receiver) = channel(1);

        self.subscribers.write().await.insert(id, MessagesSubscriber { topic, sender });

        Ok(receiver)
    }

    pub(super) async fn remove_subscriber(&self, id: usize) {
        self.subscribers.write().await.remove(&id);
    }
}

#[must_use = "Service does nothing unless polled"]
pub struct Service {
    pool: Pool<Sqlite>,
    subs_manager: Arc<MessageManager>,
    model_cache: Arc<ModelCache>,
    simple_broker: Pin<Box<dyn Stream<Item = Message> + Send>>,
}

impl Service {
    pub fn new(
        pool: Pool<Sqlite>,
        subs_manager: Arc<MessageManager>,
        model_cache: Arc<ModelCache>,
    ) -> Self {
        Self {
            pool,
            subs_manager,
            model_cache,
            simple_broker: Box::pin(SimpleBroker::<Message>::subscribe()),
        }
    }

    async fn publish_updates(
        subs: Arc<MessageManager>,
        cache: Arc<ModelCache>,
        pool: Pool<Sqlite>,
        topic: &str,
    ) -> Result<(), Error> {
        let mut closed_stream = Vec::new();

        for (idx, sub) in subs.subscribers.read().await.iter() {
            if sub.topic != topic {
                continue;
            }

            let models_query = r#"
                    SELECT group_concat(message_model.model_id) as model_names
                    FROM messages
                    JOIN entity_model ON entities.id = entity_model.entity_id
                    WHERE messages.topic = ?
                    GROUP BY messages.topic
                "#;
            let (model_names,): (String,) =
                sqlx::query_as(models_query).bind(topic).fetch_one(&pool).await?;
            let model_names: Vec<&str> = model_names.split(',').collect();
            let schemas = cache.schemas(model_names).await?;

            let entity_query = format!("{} WHERE messages.topic = ?", build_sql_query(&schemas)?);
            let row = sqlx::query(&entity_query).bind(topic).fetch_one(&pool).await?;

            let models = schemas
                .iter()
                .map(|s| {
                    let mut struct_ty = s.as_struct().expect("schema should be struct").to_owned();
                    map_row_to_ty(&s.name(), &mut struct_ty, &row)?;

                    Ok(struct_ty.try_into().unwrap())
                })
                .collect::<Result<Vec<_>, Error>>()?;

            let hashed_keys = FieldElement::from_str(&topic).map_err(ParseError::FromStr)?;
            let resp = proto::world::SubscribeMessagesResponse {
                message: Some(proto::types::Message {
                    hashed_keys: hashed_keys.to_bytes_be().to_vec(),
                    topic: topic.to_string(),
                    models,
                }),
            };

            if sub.sender.send(Ok(resp)).await.is_err() {
                closed_stream.push(*idx);
            }
        }

        for id in closed_stream {
            trace!(target = "subscription", "closing entity stream idx: {id}");
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
                if let Err(e) = Service::publish_updates(subs, cache, pool, &entity.id).await {
                    error!(target = "subscription", "error when publishing entity update: {e}");
                }
            });
        }

        Poll::Pending
    }
}
