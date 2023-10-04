use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dojo_types::WorldMetadata;
use parking_lot::RwLock;
use starknet::core::utils::parse_cairo_short_string;
use starknet_crypto::FieldElement;

use super::error::Error;
use crate::utils::compute_all_storage_addresses;

pub type EntityKeys = Vec<FieldElement>;

pub type StorageKey = FieldElement;
pub type StorageValue = FieldElement;

/// An in-memory storage for storing the component values of entities.
// TODO: check if we can use sql db instead.
pub(crate) struct ModelStorage {
    metadata: Arc<RwLock<WorldMetadata>>,
    pub(crate) storage: RwLock<HashMap<StorageKey, StorageValue>>,
    model_index: RwLock<HashMap<FieldElement, HashSet<EntityKeys>>>,
}

impl ModelStorage {
    pub(super) fn new(metadata: Arc<RwLock<WorldMetadata>>) -> Self {
        Self { metadata, storage: Default::default(), model_index: Default::default() }
    }

    #[allow(unused)]
    pub(super) fn set_entity(
        &self,
        model: FieldElement,
        raw_keys: Vec<FieldElement>,
        raw_values: Vec<FieldElement>,
    ) -> Result<(), Error> {
        let model_name = parse_cairo_short_string(&model).expect("valid cairo short string");
        let model_packed_size = self
            .metadata
            .read()
            .model(&model_name)
            .map(|model| model.packed_size)
            .ok_or(Error::UnknownModel(model_name.clone()))?;

        match raw_values.len().cmp(&(model_packed_size as usize)) {
            Ordering::Greater | Ordering::Less => {
                return Err(Error::InvalidModelValuesLen {
                    model: model_name,
                    actual_value_len: raw_values.len(),
                    expected_value_len: model_packed_size as usize,
                });
            }

            Ordering::Equal => {}
        }

        self.index_entity(model, raw_keys.clone());

        let storage_addresses = compute_all_storage_addresses(model, &raw_keys, model_packed_size);
        storage_addresses.into_iter().zip(raw_values).for_each(|(storage_address, value)| {
            self.storage.write().insert(storage_address, value);
        });

        Ok(())
    }

    pub(super) fn get_entity(
        &self,
        model: FieldElement,
        raw_keys: &[FieldElement],
    ) -> Result<Option<Vec<FieldElement>>, Error> {
        let model_name = parse_cairo_short_string(&model).expect("valid cairo short string");
        let model_packed_size = self
            .metadata
            .read()
            .model(&parse_cairo_short_string(&model).expect("valid cairo short string"))
            .map(|c| c.packed_size)
            .ok_or(Error::UnknownModel(model_name))?;

        let storage_addresses = compute_all_storage_addresses(model, raw_keys, model_packed_size);
        let values = storage_addresses
            .into_iter()
            .map(|storage_address| self.storage.read().get(&storage_address).copied())
            .collect::<Option<Vec<_>>>();

        Ok(values)
    }

    fn index_entity(&self, model: FieldElement, raw_keys: Vec<FieldElement>) {
        self.model_index.write().entry(model).or_insert_with(HashSet::new).insert(raw_keys);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use dojo_types::WorldMetadata;
    use parking_lot::RwLock;
    use starknet::core::utils::cairo_short_string_to_felt;
    use starknet::macros::felt;

    use crate::client::error::Error;
    use crate::utils::compute_all_storage_addresses;

    fn create_dummy_metadata() -> WorldMetadata {
        let models = HashMap::from([(
            "Position".into(),
            dojo_types::schema::ModelMetadata {
                name: "Position".into(),
                class_hash: felt!("1"),
                packed_size: 4,
                unpacked_size: 4,
            },
        )]);

        WorldMetadata { models, ..Default::default() }
    }

    fn create_dummy_storage() -> super::ModelStorage {
        let metadata = Arc::new(RwLock::new(create_dummy_metadata()));
        super::ModelStorage::new(metadata)
    }

    #[test]
    fn err_if_set_values_too_many() {
        let storage = create_dummy_storage();
        let entity = dojo_types::schema::EntityModel {
            model: "Position".into(),
            keys: vec![felt!("0x12345")],
        };

        let values = vec![felt!("1"), felt!("2"), felt!("3"), felt!("4"), felt!("5")];
        let model = cairo_short_string_to_felt(&entity.model).unwrap();
        let result = storage.set_entity(model, entity.keys, values);

        assert!(storage.storage.read().is_empty());
        matches!(
            result,
            Err(Error::InvalidModelValuesLen { actual_value_len: 5, expected_value_len: 4, .. })
        );
    }

    #[test]
    fn err_if_set_values_too_few() {
        let storage = create_dummy_storage();
        let entity = dojo_types::schema::EntityModel {
            model: "Position".into(),
            keys: vec![felt!("0x12345")],
        };

        let values = vec![felt!("1"), felt!("2")];
        let model = cairo_short_string_to_felt(&entity.model).unwrap();
        let result = storage.set_entity(model, entity.keys, values);

        assert!(storage.storage.read().is_empty());
        matches!(
            result,
            Err(Error::InvalidModelValuesLen { actual_value_len: 2, expected_value_len: 4, .. })
        );
    }

    #[test]
    fn set_and_get_entity_value() {
        let storage = create_dummy_storage();
        let entity = dojo_types::schema::EntityModel {
            model: "Position".into(),
            keys: vec![felt!("0x12345")],
        };

        assert!(storage.storage.read().is_empty(), "storage must be empty initially");

        let model = storage.metadata.read().model(&entity.model).cloned().unwrap();

        let expected_storage_addresses = compute_all_storage_addresses(
            cairo_short_string_to_felt(&model.name).unwrap(),
            &entity.keys,
            model.packed_size,
        );

        let expected_values = vec![felt!("1"), felt!("2"), felt!("3"), felt!("4")];
        let model_name_in_felt = cairo_short_string_to_felt(&entity.model).unwrap();

        storage
            .set_entity(model_name_in_felt, entity.keys.clone(), expected_values.clone())
            .expect("set storage values");

        let actual_values = storage
            .get_entity(model_name_in_felt, &entity.keys)
            .expect("model exist")
            .expect("values are set");

        let actual_storage_addresses =
            storage.storage.read().clone().into_keys().collect::<Vec<_>>();

        assert!(
            storage
                .model_index
                .read()
                .get(&model_name_in_felt)
                .is_some_and(|e| e.contains(&entity.keys)),
            "entity keys must be indexed"
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
