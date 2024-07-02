use std::cell::RefCell;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::task::Poll;

use dojo_types::WorldMetadata;
use futures::channel::mpsc::{self, Receiver, Sender};
use futures_util::StreamExt;
use parking_lot::{Mutex, RwLock};
use starknet::core::types::{StateDiff, StateUpdate};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet_crypto::FieldElement;
use torii_grpc::client::ModelDiffsStreaming;
use torii_grpc::types::ModelKeysClause;

use crate::client::error::{Error, ParseError};
use crate::client::storage::ModelStorage;
use crate::utils::compute_all_storage_addresses;

pub enum SubscriptionEvent {
    UpdateSubsciptionStream(ModelDiffsStreaming),
}

pub struct SubscribedModels {
    metadata: Arc<RwLock<WorldMetadata>>,
    pub(crate) models_keys: RwLock<HashSet<ModelKeysClause>>,
    /// All the relevant storage addresses derived from the subscribed models
    pub(crate) subscribed_storage_addresses: RwLock<HashSet<FieldElement>>,
}

impl SubscribedModels {
    pub(crate) fn is_synced(&self, keys: &ModelKeysClause) -> bool {
        self.models_keys.read().contains(keys)
    }

    pub(crate) fn new(metadata: Arc<RwLock<WorldMetadata>>) -> Self {
        Self {
            metadata,
            models_keys: Default::default(),
            subscribed_storage_addresses: Default::default(),
        }
    }

    pub(crate) fn add_models(&self, models_keys: Vec<ModelKeysClause>) -> Result<(), Error> {
        for keys in models_keys {
            Self::add_model(self, keys)?;
        }
        Ok(())
    }

    pub(crate) fn remove_models(&self, entities_keys: Vec<ModelKeysClause>) -> Result<(), Error> {
        for keys in entities_keys {
            Self::remove_model(self, keys)?;
        }
        Ok(())
    }

    pub(crate) fn add_model(&self, keys: ModelKeysClause) -> Result<(), Error> {
        if !self.models_keys.write().insert(keys.clone()) {
            return Ok(());
        }

        let model_packed_size = self
            .metadata
            .read()
            .models
            .get(&keys.model)
            .map(|c| c.packed_size)
            .ok_or(Error::UnknownModel(keys.model.clone()))?;

        let storage_addresses = compute_all_storage_addresses(
            cairo_short_string_to_felt(&keys.model).map_err(ParseError::CairoShortStringToFelt)?,
            &keys.keys,
            model_packed_size,
        );

        let storage_lock = &mut self.subscribed_storage_addresses.write();
        storage_addresses.into_iter().for_each(|address| {
            storage_lock.insert(address);
        });

        Ok(())
    }

    pub(crate) fn remove_model(&self, keys: ModelKeysClause) -> Result<(), Error> {
        if !self.models_keys.write().remove(&keys) {
            return Ok(());
        }

        let model_packed_size = self
            .metadata
            .read()
            .models
            .get(&keys.model)
            .map(|c| c.packed_size)
            .ok_or(Error::UnknownModel(keys.model.clone()))?;

        let storage_addresses = compute_all_storage_addresses(
            cairo_short_string_to_felt(&keys.model).map_err(ParseError::CairoShortStringToFelt)?,
            &keys.keys,
            model_packed_size,
        );

        let storage_lock = &mut self.subscribed_storage_addresses.write();
        storage_addresses.iter().for_each(|address| {
            storage_lock.remove(address);
        });

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct SubscriptionClientHandle(Mutex<Sender<SubscriptionEvent>>);

impl SubscriptionClientHandle {
    fn new(sender: Sender<SubscriptionEvent>) -> Self {
        Self(Mutex::new(sender))
    }

    pub(crate) fn update_subscription_stream(&self, stream: ModelDiffsStreaming) {
        let _ = self.0.lock().try_send(SubscriptionEvent::UpdateSubsciptionStream(stream));
    }
}

#[must_use = "SubscriptionClient does nothing unless polled"]
pub struct SubscriptionService {
    req_rcv: Receiver<SubscriptionEvent>,
    /// Model Diff stream by subscription server to receive response
    model_diffs_stream: RefCell<Option<ModelDiffsStreaming>>,

    /// Callback to be called on error
    err_callback: Option<Box<dyn Fn(tonic::Status) + Send + Sync>>,

    // for processing the model diff and updating the storage
    storage: Arc<ModelStorage>,
    world_metadata: Arc<RwLock<WorldMetadata>>,
    subscribed_models: Arc<SubscribedModels>,
}

impl SubscriptionService {
    pub(crate) fn new(
        storage: Arc<ModelStorage>,
        world_metadata: Arc<RwLock<WorldMetadata>>,
        subscribed_models: Arc<SubscribedModels>,
        model_diffs_stream: ModelDiffsStreaming,
    ) -> (Self, SubscriptionClientHandle) {
        let (req_sender, req_rcv) = mpsc::channel(128);

        let handle = SubscriptionClientHandle::new(req_sender);
        let model_diffs_stream = RefCell::new(Some(model_diffs_stream));

        let client = Self {
            req_rcv,
            storage,
            world_metadata,
            model_diffs_stream,
            err_callback: None,
            subscribed_models,
        };

        (client, handle)
    }

    // TODO: handle the subscription events properly
    fn handle_event(&self, event: SubscriptionEvent) -> Result<(), Error> {
        match event {
            SubscriptionEvent::UpdateSubsciptionStream(stream) => {
                self.model_diffs_stream.replace(Some(stream));
            }
        }
        Ok(())
    }

    // handle the response from the subscription stream
    fn handle_response(&mut self, response: Result<StateUpdate, tonic::Status>) {
        match response {
            Ok(update) => {
                self.process_model_diff(update.state_diff);
            }

            Err(err) => {
                if let Some(ref callback) = self.err_callback {
                    callback(err)
                }
            }
        }
    }

    fn process_model_diff(&mut self, diff: StateDiff) {
        let storage_entries = diff.storage_diffs.into_iter().find_map(|d| {
            let expected = self.world_metadata.read().world_address;
            let current = d.address;
            if current == expected { Some(d.storage_entries) } else { None }
        });

        let Some(entries) = storage_entries else {
            return;
        };

        let entries: Vec<(FieldElement, FieldElement)> = {
            let subscribed_models = self.subscribed_models.subscribed_storage_addresses.read();
            entries
                .into_iter()
                .filter(|entry| subscribed_models.contains(&entry.key))
                .map(|entry| (entry.key, entry.value))
                .collect()
        };

        self.storage.set_storages_at(entries);
    }
}

impl Future for SubscriptionService {
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

            if let Some(stream) = pin.model_diffs_stream.get_mut() {
                match stream.poll_next_unpin(cx) {
                    Poll::Ready(Some(res)) => pin.handle_response(res),
                    Poll::Ready(None) => return Poll::Ready(()),
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use dojo_types::schema::Ty;
    use dojo_types::WorldMetadata;
    use parking_lot::RwLock;
    use starknet::core::utils::cairo_short_string_to_felt;
    use starknet::macros::felt;
    use torii_grpc::types::ModelKeysClause;

    use crate::utils::compute_all_storage_addresses;

    fn create_dummy_metadata() -> WorldMetadata {
        let components = HashMap::from([(
            "Position".into(),
            dojo_types::schema::ModelMetadata {
                name: "Position".into(),
                class_hash: felt!("1"),
                contract_address: felt!("2"),
                packed_size: 1,
                unpacked_size: 2,
                layout: vec![],
                schema: Ty::Primitive(dojo_types::primitive::Primitive::Bool(None)),
            },
        )]);

        WorldMetadata { models: components, ..Default::default() }
    }

    #[test]
    fn add_and_remove_subscribed_model() {
        let model_name = String::from("Position");
        let keys = vec![felt!("0x12345")];
        let packed_size: u32 = 1;

        let mut expected_storage_addresses = compute_all_storage_addresses(
            cairo_short_string_to_felt(&model_name).unwrap(),
            &keys,
            packed_size,
        )
        .into_iter();

        let metadata = self::create_dummy_metadata();

        let keys = ModelKeysClause { model: model_name, keys };

        let subscribed_models = super::SubscribedModels::new(Arc::new(RwLock::new(metadata)));
        subscribed_models.add_models(vec![keys.clone()]).expect("able to add model");

        let actual_storage_addresses_count =
            subscribed_models.subscribed_storage_addresses.read().len();
        let actual_storage_addresses =
            subscribed_models.subscribed_storage_addresses.read().clone();

        assert!(subscribed_models.models_keys.read().contains(&keys));
        assert_eq!(actual_storage_addresses_count, expected_storage_addresses.len());
        assert!(expected_storage_addresses.all(|addr| actual_storage_addresses.contains(&addr)));

        subscribed_models.remove_models(vec![keys.clone()]).expect("able to remove entities");

        let actual_storage_addresses_count_after =
            subscribed_models.subscribed_storage_addresses.read().len();

        assert_eq!(actual_storage_addresses_count_after, 0);
        assert!(!subscribed_models.models_keys.read().contains(&keys));
    }
}
