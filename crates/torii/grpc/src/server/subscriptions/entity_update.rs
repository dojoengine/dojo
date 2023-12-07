use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::sync::Arc;
use std::task::{Poll, Context};
use std::pin::Pin;

use futures_util::StreamExt;
use rand::Rng;

use sqlx::{Pool, Sqlite};
use starknet::macros::short_string;
use starknet::providers::Provider;
use starknet_crypto::{poseidon_hash_many, FieldElement};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use torii_core::error::{Error, ParseError};
use torii_core::simple_broker::SimpleBroker;
use torii_core::types::Entity;
use futures::Stream;
use tracing::{debug, error, trace};

use super::error::SubscriptionError;
use crate::proto;

pub struct EntitiesSubscriber {
    /// Entity ids that the subscriber is interested in
    ids: HashSet<FieldElement>,
    /// The channel to send the response back to the subscriber.
    sender: Sender<Result<proto::world::SubscribeEntityResponse, tonic::Status>>,
}

#[derive(Default)]
pub struct EntitySubscriberManager {
    subscribers: RwLock<HashMap<usize, EntitiesSubscriber>>,
}

impl EntitySubscriberManager {
    pub async fn add_subscriber(
        &self,
        ids: Vec<FieldElement>
    ) -> Result<Receiver<Result<proto::world::SubscribeEntityResponse, tonic::Status>>, Error> {
        let id = rand::thread_rng().gen::<usize>();
        let (sender, receiver) = channel(1);

        self.subscribers.write().await.insert(
            id,
            EntitiesSubscriber { ids: ids.iter().cloned().collect(), sender }
        );

        Ok(receiver)
    }
}

#[must_use = "Service does nothing unless polled"]
pub struct Service {
    pool: Pool<Sqlite>,
    simple_broker: Pin<Box<dyn Stream<Item = Entity> + Send>>,
    entity_update_queue: VecDeque<FieldElement>
}

type PublishEntityUpdateResult = Result<(), SubscriptionError>;

impl Service {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            simple_broker: Box::pin(SimpleBroker::<Entity>::subscribe()),
            entity_update_queue: VecDeque::new()
        }
    }

    async fn publish_entity_updates(subs: Arc<EntitySubscriberManager>, id: FieldElement) {
        
    }
}

impl Future for Service {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>
    ) -> std::task::Poll<Self::Output> {
        let pin = self.get_mut();

        while let Poll::Ready(Some(entity)) = pin.simple_broker.poll_next_unpin(cx) {
            println!("GOT IT!");
        }

        Poll::Pending
    }
}