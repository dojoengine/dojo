use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use torii_client::storage::{component_storage_base_address, EntityStorage};

/// Simple in memory implementation of [EntityStorage]
#[derive(Serialize, Deserialize)]
pub struct InMemoryStorage {
    /// storage key -> Component value
    pub inner: HashMap<FieldElement, FieldElement>,
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self { inner: HashMap::new() }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InMemoryStorageError {}

// Example implementation of [EntityStorage]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl EntityStorage for InMemoryStorage {
    type Error = InMemoryStorageError;

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
