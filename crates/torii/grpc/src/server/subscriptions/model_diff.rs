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
use crate::proto::world::SubscribeModelsResponse;
use crate::types::ModelKeysClause;

pub(crate) const LOG_TARGET: &str = "torii::grpc::server::subscriptions::model_diff";

pub struct ModelMetadata {
    pub name: FieldElement,
    pub packed_size: usize,
}

pub struct ModelDiffRequest {
    pub model: ModelMetadata,
    pub keys: proto::types::ModelKeysClause,
}

impl ModelDiffRequest {}

pub struct ModelDiffSubscriber {
    /// The storage addresses that the subscriber is interested in.
    storage_addresses: HashSet<FieldElement>,
    /// The channel to send the response back to the subscriber.
    sender: Sender<Result<proto::world::SubscribeModelsResponse, tonic::Status>>,
}

#[derive(Default)]
pub struct StateDiffManager {
    subscribers: RwLock<HashMap<usize, ModelDiffSubscriber>>,
}

impl StateDiffManager {
    pub async fn add_subscriber(
        &self,
        reqs: Vec<ModelDiffRequest>,
    ) -> Result<Receiver<Result<proto::world::SubscribeModelsResponse, tonic::Status>>, Error> {
        let id: usize = rand::thread_rng().gen::<usize>();

        let (sender, receiver) = channel(1);

        // convert the list of entites into a list storage addresses
        let storage_addresses = reqs
            .into_iter()
            .map(|req| {
                let keys: ModelKeysClause =
                    req.keys.try_into().map_err(ParseError::FromByteSliceError)?;

                let base = poseidon_hash_many(&[
                    short_string!("dojo_storage"),
                    req.model.name,
                    poseidon_hash_many(&keys.keys),
                ]);

                let res = (0..req.model.packed_size)
                    .into_par_iter()
                    .map(|i| base + i.into())
                    .collect::<Vec<FieldElement>>();

                Ok(res)
            })
            .collect::<Result<Vec<_>, Error>>()?
            .into_iter()
            .flatten()
            .collect::<HashSet<FieldElement>>();

        // NOTE: unlock issue with firefox/safari
        // initially send empty stream message to return from
        // initial subscribe call
        let _ = sender.send(Ok(SubscribeModelsResponse { model_update: None })).await;

        self.subscribers
            .write()
            .await
            .insert(id, ModelDiffSubscriber { storage_addresses, sender });

        Ok(receiver)
    }

    pub(super) async fn remove_subscriber(&self, id: usize) {
        self.subscribers.write().await.remove(&id);
    }
}

type PublishStateUpdateResult = Result<(), SubscriptionError>;
type RequestStateUpdateResult = Result<MaybePendingStateUpdate, SubscriptionError>;

#[must_use = "Service does nothing unless polled"]
pub struct Service<P: Provider> {
    world_address: FieldElement,
    idle_provider: Option<P>,
    block_num_rcv: Receiver<u64>,
    state_update_queue: VecDeque<u64>,
    state_update_req_fut: Option<BoxFuture<'static, (P, u64, RequestStateUpdateResult)>>,
    subs_manager: Arc<StateDiffManager>,
    publish_fut: Option<BoxFuture<'static, PublishStateUpdateResult>>,
}

impl<P> Service<P>
where
    P: Provider + Send,
{
    pub fn new_with_block_rcv(
        block_num_rcv: Receiver<u64>,
        world_address: FieldElement,
        provider: P,
        subs_manager: Arc<StateDiffManager>,
    ) -> Self {
        Self {
            subs_manager,
            world_address,
            block_num_rcv,
            publish_fut: None,
            state_update_req_fut: None,
            idle_provider: Some(provider),
            state_update_queue: VecDeque::new(),
        }
    }

    async fn fetch_state_update(provider: P, block_num: u64) -> (P, u64, RequestStateUpdateResult) {
        let res = provider
            .get_state_update(BlockId::Number(block_num))
            .await
            .map_err(SubscriptionError::Provider);
        (provider, block_num, res)
    }

    async fn publish_updates(
        subs: Arc<StateDiffManager>,
        contract_address: FieldElement,
        state_update: StateUpdate,
    ) -> PublishStateUpdateResult {
        let mut closed_stream = Vec::new();

        let Some(ContractStorageDiffItem { storage_entries: diff_entries, .. }) =
            state_update.state_diff.storage_diffs.iter().find(|d| d.address == contract_address)
        else {
            return Ok(());
        };

        for (idx, sub) in subs.subscribers.read().await.iter() {
            let relevant_storage_entries = diff_entries
                .iter()
                .filter(|entry| sub.storage_addresses.contains(&entry.key))
                .map(|entry| {
                    let StorageEntry { key, value } = entry;
                    proto::types::StorageEntry {
                        key: format!("{key:#x}"),
                        value: format!("{value:#x}"),
                    }
                })
                .collect::<Vec<proto::types::StorageEntry>>();

            let model_update = proto::types::ModelUpdate {
                block_hash: format!("{:#x}", state_update.block_hash),
                model_diff: Some(proto::types::ModelDiff {
                    storage_diffs: vec![proto::types::StorageDiff {
                        address: format!("{contract_address:#x}"),
                        storage_entries: relevant_storage_entries,
                    }],
                }),
            };

            let resp = proto::world::SubscribeModelsResponse { model_update: Some(model_update) };

            if sub.sender.send(Ok(resp)).await.is_err() {
                closed_stream.push(*idx);
            }
        }

        for id in closed_stream {
            trace!(target = LOG_TARGET, id = %id, "Closing stream.");
            subs.remove_subscriber(id).await;
        }

        Ok(())
    }
}

/// And endless future that will listen to incoming blocks, and request the corresponding state
/// updates.
impl<P> Future for Service<P>
where
    P: Provider + Unpin + Send + Sync + 'static,
{
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let pin = self.get_mut();

        while let Poll::Ready(Some(block_num)) = pin.block_num_rcv.poll_recv(cx) {
            // queue block for requesting state updates
            pin.state_update_queue.push_back(block_num);
        }

        if let Some(provider) = pin.idle_provider.take() {
            if let Some(block_num) = pin.state_update_queue.pop_front() {
                debug!(target = LOG_TARGET, block_number = %block_num, "Fetching state update.");
                pin.state_update_req_fut =
                    Some(Box::pin(Self::fetch_state_update(provider, block_num)));
            } else {
                pin.idle_provider = Some(provider);
            }
        }

        if let Some(mut fut) = pin.state_update_req_fut.take() {
            if let Poll::Ready((provider, block_num, state_update)) = fut.poll_unpin(cx) {
                pin.idle_provider = Some(provider);

                match state_update {
                    Ok(MaybePendingStateUpdate::Update(state_update)) => {
                        pin.publish_fut = Some(Box::pin(Self::publish_updates(
                            Arc::clone(&pin.subs_manager),
                            pin.world_address,
                            state_update,
                        )));
                    }

                    Ok(MaybePendingStateUpdate::PendingUpdate(_)) => {
                        debug!(target = LOG_TARGET, block_number = %block_num, "Ignoring pending state update.")
                    }

                    Err(e) => {
                        error!(
                            target = LOG_TARGET,
                            block_num = %block_num,
                            error = %e,
                            "Fetching state update for block."
                        );
                    }
                }
            } else {
                pin.state_update_req_fut = Some(fut);
            }
        }

        if let Some(mut fut) = pin.publish_fut.take() {
            if let Poll::Ready(res) = fut.poll_unpin(cx) {
                match res {
                    Ok(_) => {
                        pin.state_update_queue.pop_front();
                    }
                    Err(e) => {
                        error!(target = LOG_TARGET, error = %e, "Publishing state update.")
                    }
                }
            } else {
                pin.publish_fut = Some(fut);
            }
        }

        Poll::Pending
    }
}
