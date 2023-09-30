//! TODO: move the subscription to a separate file

use std::collections::{HashSet, VecDeque};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::Future;
use futures_util::FutureExt;
use protos::types::maybe_pending_entity_update::Update;
use protos::world::SubscribeEntitiesResponse;
use rayon::prelude::*;
use starknet::core::types::{BlockId, ContractStorageDiffItem, MaybePendingStateUpdate};
use starknet::macros::short_string;
use starknet::providers::{Provider, ProviderError};
use starknet_crypto::{poseidon_hash_many, FieldElement};
use tokio::sync::mpsc::{Receiver, Sender};
use tonic::Status;

use crate::protos::{self};

type GetStateUpdateResult<P> =
    Result<MaybePendingStateUpdate, ProviderError<<P as Provider>::Error>>;
type StateUpdateFuture<P> = Pin<Box<dyn Future<Output = GetStateUpdateResult<P>> + Send>>;
type PublishStateUpdateFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

pub struct ModelMetadata {
    pub name: FieldElement,
    pub len: usize,
}

pub struct Entity {
    pub model: ModelMetadata,
    pub keys: Vec<FieldElement>,
}

pub struct EntityModelRequest {
    pub world: FieldElement,
    pub entities: Vec<Entity>,
}

pub struct Subscriber {
    /// The world address that the subscriber is interested in.
    world: FieldElement,
    /// The storage addresses that the subscriber is interested in.
    storage_addresses: HashSet<FieldElement>,
    /// The channel to send the response back to the subscriber.
    sender: Sender<Result<SubscribeEntitiesResponse, Status>>,
}

pub struct SubscriberManager {
    /// (set of storage addresses they care about, sender channel to send back the response)
    pub subscribers: Vec<Arc<Subscriber>>,
}

impl SubscriberManager {
    pub fn new() -> Self {
        Self { subscribers: Vec::default() }
    }

    fn add_subscriber(
        &mut self,
        request: (EntityModelRequest, Sender<Result<SubscribeEntitiesResponse, Status>>),
    ) {
        let (EntityModelRequest { world, entities }, sender) = request;

        // convert the list of entites into a list storage addresses
        let storage_addresses = entities
            .par_iter()
            .map(|entity| {
                let base = poseidon_hash_many(&[
                    short_string!("dojo_storage"),
                    entity.model.name,
                    poseidon_hash_many(&entity.keys),
                ]);

                (0..entity.model.len)
                    .into_par_iter()
                    .map(|i| base + i.into())
                    .collect::<Vec<FieldElement>>()
            })
            .flatten()
            .collect::<HashSet<FieldElement>>();

        self.subscribers.push(Arc::new(Subscriber { world, storage_addresses, sender }))
    }
}

impl Default for SubscriberManager {
    fn default() -> Self {
        Self::new()
    }
}

/// a service which handles entity subscription requests. it is an endless future where it awaits
/// for new blocks, fetch its state update, and publish them to the subscribers.
pub struct EntitySubscriptionService<P: Provider> {
    /// A channel to communicate with the indexer engine, in order to receive the block number that
    /// the indexer engine is processing at any moment. This way, we can sync with the indexer and
    /// request the state update of the current block that the indexer is currently processing.
    block_rx: Receiver<u64>,
    /// The Starknet provider.
    provider: Arc<P>,
    /// A list of state update futures, each corresponding to a block number that was received from
    /// the indexer engine.
    state_update_req_futs: VecDeque<(u64, StateUpdateFuture<P>)>,

    publish_update_fut: Option<PublishStateUpdateFuture>,

    block_num_queue: Vec<u64>,
    /// Receive subscribers from gRPC server.
    /// This receives streams of (sender channel, list of entities to subscribe) tuple
    subscriber_recv:
        Receiver<(EntityModelRequest, Sender<Result<SubscribeEntitiesResponse, Status>>)>,

    subscriber_manager: SubscriberManager,
}

impl<P> EntitySubscriptionService<P>
where
    P: Provider,
{
    pub fn new(
        provider: P,
        subscriber_recv: Receiver<(
            EntityModelRequest,
            Sender<Result<SubscribeEntitiesResponse, Status>>,
        )>,
        block_rx: Receiver<u64>,
    ) -> Self {
        Self {
            block_rx,
            subscriber_recv,
            provider: Arc::new(provider),
            block_num_queue: Default::default(),
            publish_update_fut: Default::default(),
            state_update_req_futs: Default::default(),
            subscriber_manager: SubscriberManager::new(),
        }
    }

    /// Process the fetched state update, and publish to the subscribers, the relevant values for
    /// them.
    async fn publish_state_updates_to_subscribers(
        subscribers: Vec<Arc<Subscriber>>,
        state_update: MaybePendingStateUpdate,
    ) {
        let state_diff = match &state_update {
            MaybePendingStateUpdate::PendingUpdate(update) => &update.state_diff,
            MaybePendingStateUpdate::Update(update) => &update.state_diff,
        };

        // iterate over the list of subscribers, and construct the relevant state diffs for each
        // subscriber
        for sub in subscribers {
            // if there is no state diff for the current world, then skip, otherwise, extract the
            // state diffs of the world
            let Some(ContractStorageDiffItem { storage_entries: diff_entries, .. }) =
                state_diff.storage_diffs.iter().find(|d| d.address == sub.world)
            else {
                continue;
            };

            let relevant_storage_entries = diff_entries
                .iter()
                .filter(|entry| sub.storage_addresses.contains(&entry.key))
                .map(|entry| protos::types::StorageEntry {
                    key: format!("{:#x}", entry.key),
                    value: format!("{:#x}", entry.value),
                })
                .collect::<Vec<protos::types::StorageEntry>>();

            // if there is no state diffs relevant to the current subscriber, then skip
            if relevant_storage_entries.is_empty() {
                continue;
            }

            let response = SubscribeEntitiesResponse {
                entity_update: Some(protos::types::MaybePendingEntityUpdate {
                    update: Some(match &state_update {
                        MaybePendingStateUpdate::PendingUpdate(_) => {
                            Update::PendingEntityUpdate(protos::types::PendingEntityUpdate {
                                entity_diff: Some(protos::types::EntityDiff {
                                    storage_diffs: vec![protos::types::StorageDiff {
                                        address: format!("{:#x}", sub.world),
                                        storage_entries: relevant_storage_entries,
                                    }],
                                }),
                            })
                        }

                        MaybePendingStateUpdate::Update(update) => {
                            Update::EntityUpdate(protos::types::EntityUpdate {
                                block_hash: format!("{:#x}", update.block_hash),
                                entity_diff: Some(protos::types::EntityDiff {
                                    storage_diffs: vec![protos::types::StorageDiff {
                                        address: format!("{:#x}", sub.world),
                                        storage_entries: relevant_storage_entries,
                                    }],
                                }),
                            })
                        }
                    }),
                }),
            };

            match sub.sender.send(Ok(response)).await {
                Ok(_) => {
                    println!("state diff sent")
                }
                Err(e) => {
                    println!("stream closed: {e:?}");
                }
            }
        }
    }

    async fn do_get_state_update(provider: Arc<P>, block_number: u64) -> GetStateUpdateResult<P> {
        provider.get_state_update(BlockId::Number(block_number)).await
    }
}

// an endless future which will receive the block number from the indexer engine, and will
// request its corresponding state update.
impl<P> Future for EntitySubscriptionService<P>
where
    P: Provider + Send + Sync + Unpin + 'static,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pin = self.get_mut();

        // drain the stream
        while let Poll::Ready(Some(block_num)) = pin.block_rx.poll_recv(cx) {
            // we still need to drain the stream, even if there are no subscribers. But dont have to
            // queue for the block number
            if !pin.subscriber_manager.subscribers.is_empty() {
                pin.block_num_queue.push(block_num);
            }
        }

        // if there are any queued block numbers, then fetch the corresponding state updates
        while let Some(block_num) = pin.block_num_queue.pop() {
            let fut = Box::pin(Self::do_get_state_update(Arc::clone(&pin.provider), block_num));
            pin.state_update_req_futs.push_back((block_num, fut));
        }

        // handle incoming new subscribers
        while let Poll::Ready(Some(request)) = pin.subscriber_recv.poll_recv(cx) {
            println!("received new subscriber");
            pin.subscriber_manager.add_subscriber(request);
        }

        // check if there's ongoing publish future, if yes, poll it and if its still not ready
        // then return pending,
        // dont request for state update, since we are still waiting for the previous state update
        // to be published
        if let Some(mut fut) = pin.publish_update_fut.take() {
            if fut.poll_unpin(cx).is_pending() {
                pin.publish_update_fut = Some(fut);
                return Poll::Pending;
            }
        }

        // poll ongoing state update requests
        if let Some((block_num, mut fut)) = pin.state_update_req_futs.pop_front() {
            match fut.poll_unpin(cx) {
                Poll::Ready(Ok(state_update)) => {
                    let subscribers = pin.subscriber_manager.subscribers.clone();
                    pin.publish_update_fut = Some(Box::pin(
                        Self::publish_state_updates_to_subscribers(subscribers, state_update),
                    ));
                }

                Poll::Ready(Err(e)) => {
                    println!("error fetching state update for block {block_num}: {:?}", e)
                }

                Poll::Pending => pin.state_update_req_futs.push_back((block_num, fut)),
            }
        }

        Poll::Pending
    }
}
