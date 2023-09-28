use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::bail;
use dojo_types::WorldMetadata;
use parking_lot::RwLock;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::macros::short_string;
use starknet_crypto::{poseidon_hash_many, FieldElement};

pub type EntityKeys = Vec<FieldElement>;

pub type StorageKey = FieldElement;
pub type StorageValue = FieldElement;

/// An in-memory storage for storing the component values of entities.
/// TODO: check if we can use sql db instead.
pub(crate) struct ComponentStorage {
    metadata: Arc<RwLock<WorldMetadata>>,
    // TODO: change entity id to entity keys
    component_index: RwLock<HashMap<String, HashSet<EntityKeys>>>, /* component -> list of
                                                                    * entity ids */
    pub(crate) storage: RwLock<HashMap<StorageKey, StorageValue>>,
}

impl ComponentStorage {
    pub(crate) fn new(metadata: Arc<RwLock<WorldMetadata>>) -> Self {
        Self { metadata, storage: Default::default(), component_index: Default::default() }
    }

    pub fn set_entity(
        &self,
        key: (String, Vec<FieldElement>),
        values: Vec<FieldElement>,
    ) -> anyhow::Result<()> {
        let (component, entity_keys) = key;

        let Some(component_size) = self.metadata.read().components.get(&component).map(|c| c.size)
        else {
            bail!("unknown component: {component}")
        };

        if values.len().cmp(&(component_size as usize)).is_gt() {
            bail!("too many values for component: {component}")
        } else if values.len().cmp(&(component_size as usize)).is_lt() {
            bail!("not enough values for component: {component}")
        }

        let hashed_key = poseidon_hash_many(&entity_keys);
        let entity_id = poseidon_hash_many(&[
            short_string!("dojo_storage"),
            cairo_short_string_to_felt(&component).expect("valid cairo short string"),
            hashed_key,
        ]);

        self.component_index.write().entry(component).or_default().insert(entity_keys);

        values.into_iter().enumerate().for_each(|(i, value)| {
            self.storage.write().insert(entity_id + i.into(), value);
        });

        Ok(())
    }

    pub fn get_entity(&self, key: (String, Vec<FieldElement>)) -> Option<Vec<FieldElement>> {
        let (component, entity_keys) = key;

        let Some(component_size) = self.metadata.read().components.get(&component).map(|c| c.size)
        else {
            return None;
        };

        let hashed_key = poseidon_hash_many(&entity_keys);
        let entity_id = poseidon_hash_many(&[
            short_string!("dojo_storage"),
            cairo_short_string_to_felt(&component).expect("valid cairo short string"),
            hashed_key,
        ]);

        let mut values = Vec::with_capacity(component_size as usize);

        for i in 0..component_size {
            let value = self.storage.read().get(&(entity_id + i.into())).copied()?;
            values.push(value);
        }

        Some(values)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

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

    fn create_dummy_storage() -> super::ComponentStorage {
        let metadata = Arc::new(RwLock::new(create_dummy_metadata()));
        super::ComponentStorage::new(metadata)
    }

    #[test]
    fn err_if_set_values_too_many() {
        let storage = create_dummy_storage();
        let entity = dojo_types::model::EntityModel {
            model: "Position".into(),
            keys: vec![felt!("0x12345")],
        };

        let values = vec![felt!("1"), felt!("2"), felt!("3"), felt!("4")];
        let result = storage.set_entity((entity.model, entity.keys), values);

        assert!(result.is_err());
        assert!(storage.storage.read().is_empty());
    }

    #[test]
    fn err_if_set_values_too_few() {
        let storage = create_dummy_storage();
        let entity = dojo_types::model::EntityModel {
            model: "Position".into(),
            keys: vec![felt!("0x12345")],
        };

        let values = vec![felt!("1"), felt!("2")];
        let result = storage.set_entity((entity.model, entity.keys), values);

        assert!(result.is_err());
        assert!(storage.storage.read().is_empty());
    }

    #[test]
    fn set_and_get_entity_value() {
        let storage = create_dummy_storage();
        let entity = dojo_types::model::EntityModel {
            model: "Position".into(),
            keys: vec![felt!("0x12345")],
        };

        assert!(storage.storage.read().is_empty());

        let component = storage.metadata.read().components.get("Position").cloned().unwrap();

        let expected_storage_addresses = {
            let base = poseidon_hash_many(&[
                short_string!("dojo_storage"),
                cairo_short_string_to_felt(&entity.model).unwrap(),
                poseidon_hash_many(&entity.keys),
            ]);

            (0..component.size).map(|i| base + i.into()).collect::<Vec<_>>()
        };

        let expected_values = vec![felt!("1"), felt!("2"), felt!("3")];

        storage
            .set_entity((entity.model.clone(), entity.keys.clone()), expected_values.clone())
            .expect("set storage values");

        let actual_values =
            storage.get_entity((entity.model, entity.keys.clone())).expect("get storage values");

        let actual_storage_addresses =
            storage.storage.read().clone().into_keys().collect::<Vec<_>>();

        assert!(
            storage
                .component_index
                .read()
                .get("Position")
                .is_some_and(|e| e.contains(&entity.keys)),
            "entity keys must be indexed"
        );
        assert!(actual_values == expected_values);
        assert!(storage.storage.read().len() == component.size as usize);
        assert!(
            actual_storage_addresses
                .into_iter()
                .all(|address| expected_storage_addresses.contains(&address))
        );
    }
}
