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
use tokio::sync::mpsc::{
    channel, unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender,
};
use tokio::sync::RwLock;
use torii_core::error::{Error, ParseError};
use torii_core::simple_broker::SimpleBroker;
use torii_core::sql::FELT_DELIMITER;
use torii_core::types::Event;
use tracing::{error, trace};

use super::match_keys;
use crate::proto;
use crate::proto::world::SubscribeEventsResponse;
use crate::types::EntityKeysClause;

pub(crate) const LOG_TARGET: &str = "torii::grpc::server::subscriptions::event";

#[derive(Debug)]
pub struct EventSubscriber {
    /// Event keys that the subscriber is interested in
    keys: Vec<EntityKeysClause>,
    /// The channel to send the response back to the subscriber.
    sender: Sender<Result<proto::world::SubscribeEventsResponse, tonic::Status>>,
}

#[derive(Debug, Default)]
pub struct EventManager {
    subscribers: RwLock<HashMap<usize, EventSubscriber>>,
}

impl EventManager {
    pub async fn add_subscriber(
        &self,
        keys: Vec<EntityKeysClause>,
    ) -> Result<Receiver<Result<proto::world::SubscribeEventsResponse, tonic::Status>>, Error> {
        let id = rand::thread_rng().gen::<usize>();
        let (sender, receiver) = channel(1);

        // NOTE: unlock issue with firefox/safari
        // initially send empty stream message to return from
        // initial subscribe call
        let _ = sender.send(Ok(SubscribeEventsResponse { event: None })).await;

        self.subscribers.write().await.insert(id, EventSubscriber { keys, sender });

        Ok(receiver)
    }

    pub(super) async fn remove_subscriber(&self, id: usize) {
        self.subscribers.write().await.remove(&id);
    }
}

#[must_use = "Service does nothing unless polled"]
#[allow(missing_debug_implementations)]
pub struct Service {
    simple_broker: Pin<Box<dyn Stream<Item = Event> + Send>>,
    event_sender: UnboundedSender<Event>,
}

impl Service {
    pub fn new(subs_manager: Arc<EventManager>) -> Self {
        let (event_sender, event_receiver) = unbounded_channel();
        let service =
            Self { simple_broker: Box::pin(SimpleBroker::<Event>::subscribe()), event_sender };

        tokio::spawn(Self::publish_updates(subs_manager, event_receiver));

        service
    }

    async fn publish_updates(
        subs: Arc<EventManager>,
        mut event_receiver: UnboundedReceiver<Event>,
    ) {
        while let Some(event) = event_receiver.recv().await {
            if let Err(e) = Self::process_event(&subs, &event).await {
                error!(target = LOG_TARGET, error = %e, "Processing event update.");
            }
        }
    }

    async fn process_event(subs: &Arc<EventManager>, event: &Event) -> Result<(), Error> {
        let mut closed_stream = Vec::new();
        let keys = event
            .keys
            .trim_end_matches(FELT_DELIMITER)
            .split(FELT_DELIMITER)
            .map(Felt::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ParseError::from)?;
        let data = event
            .data
            .trim_end_matches(FELT_DELIMITER)
            .split(FELT_DELIMITER)
            .map(Felt::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(ParseError::from)?;

        for (idx, sub) in subs.subscribers.read().await.iter() {
            if !match_keys(&keys, &sub.keys) {
                continue;
            }

            let resp = proto::world::SubscribeEventsResponse {
                event: Some(proto::types::Event {
                    keys: keys.iter().map(|k| k.to_bytes_be().to_vec()).collect(),
                    data: data.iter().map(|d| d.to_bytes_be().to_vec()).collect(),
                    transaction_hash: Felt::from_str(&event.transaction_hash)
                        .map_err(ParseError::from)?
                        .to_bytes_be()
                        .to_vec(),
                }),
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
            if let Err(e) = pin.event_sender.send(event) {
                error!(target = LOG_TARGET, error = %e, "Sending event to processor.");
            }
        }

        Poll::Pending
    }
}
