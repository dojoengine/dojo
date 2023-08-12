use std::collections::HashMap;

use async_trait::async_trait;
use dojo_client::storage::{component_storage_base_address, EntityStorage};
use starknet::core::types::FieldElement;

/// Simple in memory implementation of [EntityStorage]
pub struct InMemoryStorage {
    /// storage key -> Component value
    pub inner: HashMap<FieldElement, FieldElement>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self { inner: HashMap::new() }
    }
}

// Example implementation of [EntityStorage]
#[async_trait]
impl EntityStorage for InMemoryStorage {
    type Error = ();

    async fn set(
        &mut self,
        component: FieldElement,
        keys: Vec<FieldElement>,
        values: Vec<FieldElement>,
    ) -> Result<(), Self::Error> {
        let base_address = component_storage_base_address(component, &keys);
        for (offset, value) in values.into_iter().enumerate() {
            self.inner.insert(base_address + offset.into(), value);
        }
        Ok(())
    }

    async fn get(
        &self,
        component: FieldElement,
        keys: Vec<FieldElement>,
        length: usize,
    ) -> Result<Vec<FieldElement>, Self::Error> {
        let base_address = component_storage_base_address(component, &keys);
        let mut values = Vec::with_capacity(length);
        for i in 0..length {
            let address = base_address + i.into();
            let value = self.inner.get(&address).cloned();
            values.push(value.unwrap_or(FieldElement::ZERO));
        }
        Ok(values)
    }
}
