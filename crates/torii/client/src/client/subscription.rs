use std::collections::HashSet;
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use std::task::Poll;

use anyhow::{anyhow, bail, Result};
use dojo_types::model::EntityModel;
use dojo_types::WorldMetadata;
use futures::channel::mpsc::{Receiver, Sender};
use futures_util::StreamExt;
use parking_lot::RwLock;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::macros::short_string;
use starknet_crypto::{poseidon_hash_many, FieldElement};
use torii_grpc::protos;
use torii_grpc::protos::types::EntityDiff;
use torii_grpc::protos::world::SubscribeEntitiesResponse;

use super::ComponentStorage;

#[derive(Debug, Clone)]
pub enum SubscriptionEvent {
    SubscribeEntity(EntityModel),
    UnsubscribeEntity(EntityModel),
}

pub struct SubscribedEntities {
    pub(super) entities: RwLock<HashSet<EntityModel>>,
    /// All the relevant storage addresses derived from the subscribed entities
    pub(super) subscribed_storage_addresses: RwLock<HashSet<FieldElement>>,
    metadata: Arc<RwLock<WorldMetadata>>,
}

impl SubscribedEntities {
    pub fn new(metadata: Arc<RwLock<WorldMetadata>>) -> Self {
        Self {
            metadata,
            entities: Default::default(),
            subscribed_storage_addresses: Default::default(),
        }
    }

    pub fn add_entities(&self, entities: Vec<EntityModel>) -> anyhow::Result<()> {
        for entity in entities {
            if !self.entities.write().insert(entity.clone()) {
                continue;
            }

            let hashed_key = poseidon_hash_many(&entity.keys);
            let base_address = poseidon_hash_many(&[
                short_string!("dojo_storage"),
                cairo_short_string_to_felt(&entity.model).map_err(|e| anyhow!(e))?,
                hashed_key,
            ]);

            let Some(component_size) =
                self.metadata.read().components.get(&entity.model).map(|c| c.size)
            else {
                bail!("unknown component {}", entity.model)
            };

            (0..component_size).for_each(|i| {
                self.subscribed_storage_addresses.write().insert(base_address + i.into());
            });
        }

        Ok(())
    }

    pub fn remove_entities(&self, entities: Vec<EntityModel>) -> anyhow::Result<()> {
        for entity in entities {
            if !self.entities.write().remove(&entity) {
                continue;
            }

            let hashed_key = poseidon_hash_many(&entity.keys);
            let base_address = poseidon_hash_many(&[
                short_string!("dojo_storage"),
                cairo_short_string_to_felt(&entity.model).map_err(|e| anyhow!(e))?,
                hashed_key,
            ]);

            let Some(component_size) =
                self.metadata.read().components.get(&entity.model).map(|c| c.size)
            else {
                bail!("unknown component {}", entity.model)
            };

            (0..component_size).for_each(|i| {
                self.subscribed_storage_addresses.write().remove(&(base_address + i.into()));
            });
        }

        Ok(())
    }
}

#[allow(unused)]
pub(crate) struct SubscriptionClientHandle {
    pub(super) event_handler: Sender<SubscriptionEvent>,
}

#[must_use = "SubscriptionClient does nothing unless polled"]
pub struct SubscriptionClient {
    pub(super) req_rcv: Receiver<SubscriptionEvent>,
    /// The stream returned by the subscription server to receive the response
    pub(super) sub_res_stream: tonic::Streaming<SubscribeEntitiesResponse>,
    /// Callback to be called on error
    pub(super) err_callback: Option<Box<dyn Fn(tonic::Status) + Send + Sync>>,

    // for processing the entity diff and updating the storage
    pub(super) storage: Arc<ComponentStorage>,
    pub(super) world_metadata: Arc<RwLock<WorldMetadata>>,
    pub(super) subscribed_entities: Arc<SubscribedEntities>,
}

impl SubscriptionClient {
    // TODO: handle the subscription events properly
    fn handle_event(&self, event: SubscriptionEvent) -> Result<()> {
        match event {
            SubscriptionEvent::SubscribeEntity(entity) => {
                self.subscribed_entities.add_entities(vec![entity])
            }
            SubscriptionEvent::UnsubscribeEntity(entity) => {
                self.subscribed_entities.remove_entities(vec![entity])
            }
        }
    }

    // handle the response from the subscription stream
    fn handle_response(&self, response: Result<SubscribeEntitiesResponse, tonic::Status>) {
        match response {
            Ok(res) => {
                let entity_diff = res
                    .entity_update
                    .and_then(|e| e.update)
                    .and_then(|update| match update {
                        protos::types::maybe_pending_entity_update::Update::EntityUpdate(
                            update,
                        ) => update.entity_diff,
                        protos::types::maybe_pending_entity_update::Update::PendingEntityUpdate(
                            update,
                        ) => update.entity_diff,
                    })
                    .expect("have entity update");

                self.process_entity_diff(entity_diff);
            }

            Err(err) => {
                if let Some(ref callback) = self.err_callback {
                    callback(err)
                }
            }
        }
    }

    fn process_entity_diff(&self, diff: EntityDiff) {
        let storage_entries = diff.storage_diffs.into_iter().find_map(|d| {
            let expected = self.world_metadata.read().world_address;
            let current = FieldElement::from_str(&d.address).expect("valid FieldElement value");
            if current == expected { Some(d.storage_entries) } else { None }
        });

        let Some(entries) = storage_entries else {
            return;
        };

        entries.into_iter().enumerate().for_each(|(i, entry)| {
            let key = FieldElement::from_str(&entry.key).expect("valid FieldElement value");
            let value = FieldElement::from_str(&entry.value).expect("valid FieldElement value");

            println!("[{i}] key: {key:#x} value: {value:#x}", key = key, value = value);

            if self.subscribed_entities.subscribed_storage_addresses.read().contains(&key) {
                self.storage.storage.write().insert(key, value);
            } else {
                panic!("unknown storage address");
            }
        })
    }
}

impl Future for SubscriptionClient {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let pin = self.get_mut();

        loop {
            while let Poll::Ready(Some(req)) = pin.req_rcv.poll_next_unpin(cx) {
                let _ = pin.handle_event(req);
            }

            match pin.sub_res_stream.poll_next_unpin(cx) {
                Poll::Ready(Some(res)) => pin.handle_response(res),

                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use dojo_types::model::EntityModel;
    use dojo_types::WorldMetadata;
    use parking_lot::RwLock;
    use starknet::core::utils::cairo_short_string_to_felt;
    use starknet::macros::{felt, short_string};
    use starknet_crypto::poseidon_hash_many;

    fn create_dummy_metadata() -> WorldMetadata {
        let components = HashMap::from([(
            "Position".into(),
            dojo_types::model::ModelMetadata {
                name: "Position".into(),
                class_hash: felt!("1"),
                size: 3,
            },
        )]);

        WorldMetadata { components, ..Default::default() }
    }

    #[test]
    fn add_and_remove_subscribed_entity() {
        let component_name = String::from("Position");
        let component_size: u32 = 3;
        let keys = vec![felt!("0x12345")];

        let mut expected_storage_addresses = {
            let base = poseidon_hash_many(&[
                short_string!("dojo_storage"),
                cairo_short_string_to_felt(&component_name).unwrap(),
                poseidon_hash_many(&keys),
            ]);

            (0..component_size).map(|i| base + i.into()).collect::<Vec<_>>()
        }
        .into_iter();

        let metadata = self::create_dummy_metadata();
        let entity = EntityModel { model: component_name, keys };

        let subscribed_entities = super::SubscribedEntities::new(Arc::new(RwLock::new(metadata)));
        subscribed_entities.add_entities(vec![entity.clone()]).expect("able to add entity");

        let actual_storage_addresses_count =
            subscribed_entities.subscribed_storage_addresses.read().len();
        let actual_storage_addresses =
            subscribed_entities.subscribed_storage_addresses.read().clone();

        assert!(subscribed_entities.entities.read().contains(&entity));
        assert_eq!(actual_storage_addresses_count, expected_storage_addresses.len());
        assert!(expected_storage_addresses.all(|addr| actual_storage_addresses.contains(&addr)));

        subscribed_entities.remove_entities(vec![entity.clone()]).expect("able to remove entities");

        let actual_storage_addresses_count_after =
            subscribed_entities.subscribed_storage_addresses.read().len();

        assert_eq!(actual_storage_addresses_count_after, 0);
        assert!(!subscribed_entities.entities.read().contains(&entity));
    }
}
