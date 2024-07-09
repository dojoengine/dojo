use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dojo_types::WorldMetadata;
use futures::channel::mpsc::{channel, Receiver, Sender};
use parking_lot::{Mutex, RwLock};
use starknet::core::types::Felt;

use super::error::Error;
use crate::utils::compute_all_storage_addresses;

pub type EntityKeys = Vec<Felt>;

pub type StorageKey = Felt;
pub type StorageValue = Felt;

/// An in-memory storage for storing the component values of entities.
// TODO: check if we can use sql db instead.
#[derive(Debug)]
pub struct ModelStorage {
    metadata: Arc<RwLock<WorldMetadata>>,
    storage: RwLock<HashMap<StorageKey, StorageValue>>,
    // a map of model name to a set of model keys.
    model_index: RwLock<HashMap<Felt, HashSet<EntityKeys>>>,

    // listener for storage updates.
    senders: Mutex<HashMap<u8, Sender<()>>>,
    listeners: Mutex<HashMap<StorageKey, Vec<u8>>>,
}

impl ModelStorage {
    pub(super) fn new(metadata: Arc<RwLock<WorldMetadata>>) -> Self {
        Self {
            metadata,
            storage: Default::default(),
            model_index: Default::default(),
            senders: Default::default(),
            listeners: Default::default(),
        }
    }

    /// Listen to model changes.
    ///
    /// # Arguments
    /// * `model` - the model name.
    /// * `keys` - the keys of the model.
    ///
    /// # Returns
    /// A receiver that will receive updates for the specified storage keys.
    pub fn add_listener(&self, model: Felt, keys: &[Felt]) -> Result<Receiver<()>, Error> {
        let storage_addresses = self.get_model_storage_addresses(model, keys)?;

        let (sender, receiver) = channel(128);
        let listener_id = self.senders.lock().len() as u8;
        self.senders.lock().insert(listener_id, sender);

        storage_addresses.iter().for_each(|key| {
            self.listeners.lock().entry(*key).or_default().push(listener_id);
        });

        Ok(receiver)
    }

    /// Retrieves the raw values of an model.
    pub fn get_model_storage(
        &self,
        model: Felt,
        raw_keys: &[Felt],
    ) -> Result<Option<Vec<Felt>>, Error> {
        let storage_addresses = self.get_model_storage_addresses(model, raw_keys)?;
        Ok(storage_addresses
            .into_iter()
            .map(|storage_address| self.storage.read().get(&storage_address).copied())
            .collect::<Option<Vec<_>>>())
    }

    /// Set the raw values of an model.
    pub fn set_model_storage(
        &self,
        model: Felt,
        raw_keys: Vec<Felt>,
        raw_values: Vec<Felt>,
    ) -> Result<(), Error> {
        let storage_addresses = self.get_model_storage_addresses(model, &raw_keys)?;
        self.set_storages_at(storage_addresses.into_iter().zip(raw_values).collect());
        self.index_model(model, raw_keys);

        Ok(())
    }

    /// Set the value of storage slots in bulk
    pub(super) fn set_storages_at(&self, storage_models: Vec<(Felt, Felt)>) {
        let mut senders: HashSet<u8> = Default::default();

        for (key, _) in &storage_models {
            if let Some(lists) = self.listeners.lock().get(key) {
                for id in lists {
                    senders.insert(*id);
                }
            }
        }

        self.storage.write().extend(storage_models);

        for sender_id in senders {
            self.notify_listener(sender_id);
        }
    }

    fn notify_listener(&self, id: u8) {
        if let Some(sender) = self.senders.lock().get_mut(&id) {
            let _ = sender.try_send(());
        }
    }

    fn get_model_storage_addresses(
        &self,
        model: Felt,
        raw_keys: &[Felt],
    ) -> Result<Vec<Felt>, Error> {
        let model_packed_size = self
            .metadata
            .read()
            .model(&model)
            .map(|c| c.packed_size)
            .ok_or(Error::UnknownModel(model))?;

        Ok(compute_all_storage_addresses(model, raw_keys, model_packed_size))
    }

    fn index_model(&self, model: Felt, raw_keys: Vec<Felt>) {
        self.model_index.write().entry(model).or_default().insert(raw_keys);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use dojo_types::schema::Ty;
    use dojo_types::WorldMetadata;
    use dojo_world::contracts::naming::compute_selector_from_names;
    use parking_lot::RwLock;
    use starknet::macros::felt;

    use crate::utils::compute_all_storage_addresses;

    fn create_dummy_metadata() -> WorldMetadata {
        let models = HashMap::from([(
            compute_selector_from_names("Test", "Position"),
            dojo_types::schema::ModelMetadata {
                namespace: "Test".into(),
                name: "Position".into(),
                class_hash: felt!("1"),
                contract_address: felt!("2"),
                packed_size: 4,
                unpacked_size: 4,
                layout: vec![],
                schema: Ty::Primitive(dojo_types::primitive::Primitive::Bool(None)),
            },
        )]);

        WorldMetadata { models, ..Default::default() }
    }

    fn create_dummy_storage() -> super::ModelStorage {
        let metadata = Arc::new(RwLock::new(create_dummy_metadata()));
        super::ModelStorage::new(metadata)
    }

    #[test]
    fn set_and_get_model_value() {
        let storage = create_dummy_storage();
        let keys = vec![felt!("0x12345")];

        assert!(storage.storage.read().is_empty(), "storage must be empty initially");

        let model_selector = compute_selector_from_names("Test", "Position");

        let model = storage.metadata.read().model(&model_selector).cloned().unwrap();
        let expected_storage_addresses =
            compute_all_storage_addresses(model_selector, &keys, model.packed_size);

        let expected_values = vec![felt!("1"), felt!("2"), felt!("3"), felt!("4")];

        storage
            .set_model_storage(model_selector, keys.clone(), expected_values.clone())
            .expect("set storage values");

        let actual_values: Vec<starknet::core::types::Felt> = storage
            .get_model_storage(model_selector, &keys)
            .expect("model exist")
            .expect("values are set");

        let actual_storage_addresses =
            storage.storage.read().clone().into_keys().collect::<Vec<_>>();

        assert!(
            storage.model_index.read().get(&model_selector).is_some_and(|e| e.contains(&keys)),
            "model keys must be indexed"
        );
        assert!(actual_values == expected_values);
        assert!(storage.storage.read().len() == model.packed_size as usize);
        assert!(
            actual_storage_addresses
                .into_iter()
                .all(|address| expected_storage_addresses.contains(&address))
        );
    }
}
