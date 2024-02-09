use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::sync::Arc;
use std::task::Poll;

use futures_util::future::BoxFuture;
use futures_util::FutureExt;
use rand::Rng;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};
use starknet::core::types::{
    BlockId, ContractStorageDiffItem, MaybePendingStateUpdate, StateUpdate, StorageEntry,
};
use starknet::macros::short_string;
use starknet::providers::Provider;
use starknet_crypto::{poseidon_hash_many, FieldElement};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use torii_core::error::{Error, ParseError};
use tracing::{debug, error, trace};

use super::error::SubscriptionError;
use crate::proto;
use crate::proto::relay::SubscribeMessagesResponse;
use crate::types::KeysClause;

pub struct TopicSubscriber {
    topic: String,
    sender: Sender<Result<proto::relay::SubscribeMessagesResponse, tonic::Status>>,
}

#[derive(Default)]
pub struct RelayManager {
    subscribers: RwLock<HashMap<usize, TopicSubscriber>>,
}

impl RelayManager {
    pub async fn add_subscriber(
        &self,
        req: proto::relay::SubscribeMessagesRequest,
    ) -> Result<Receiver<Result<proto::relay::SubscribeMessagesResponse, tonic::Status>>, Error>
    {
        let id: usize = rand::thread_rng().gen::<usize>();

        let (sender, receiver) = channel(1);

        self.subscribers.write().await.insert(id, TopicSubscriber { topic: req.topic, sender });

        Ok(receiver)
    }

    pub(super) async fn remove_subscriber(&self, id: usize) {
        self.subscribers.write().await.remove(&id);
    }
}

#[must_use = "Service does nothing unless polled"]
pub struct Service {
    subs_manager: Arc<RelayManager>,
    message_rcv: Receiver<proto::relay::Message>,
}

impl Service {
    pub fn new(
        subs_manager: Arc<RelayManager>,
        message_rcv: Receiver<proto::relay::Message>,
    ) -> Self {
        Self { subs_manager, message_rcv }
    }
}

impl Future for Service {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let pin = self.get_mut();

        while let Poll::Ready(Some(msg)) = pin.message_rcv.poll_recv(cx) {
            let subs = pin.subs_manager.clone();
            let topic = msg.topic.clone();
            let message = msg.clone();

            tokio::spawn(async move {
                let mut closed_streams = Vec::new();
                for (idx, sub) in subs.subscribers.read().await.iter() {
                    if sub.topic == topic {
                        if let Err(e) = sub.sender.send(Ok(SubscribeMessagesResponse {
                            message: Some(message.clone()),
                        })).await {
                            error!("Error sending message to subscriber: {:?}", e);
                            closed_streams.push(*idx);
                        }
                    }
                }

                for id in closed_streams {
                    trace!("closing relay stream idx: {}", id);
                    subs.remove_subscriber(id).await
                }
            });
        }

        Poll::Pending
    }
}
