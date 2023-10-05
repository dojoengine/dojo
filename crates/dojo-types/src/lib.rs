use std::collections::HashMap;

use schema::ModelMetadata;
use serde::Serialize;
use starknet::core::types::FieldElement;

pub mod event;
pub mod packing;
pub mod primitive;
pub mod schema;
pub mod storage;
pub mod system;

/// Represents the metadata of a World
#[derive(Debug, Clone, Serialize, Default)]
pub struct WorldMetadata {
    pub world_address: FieldElement,
    pub world_class_hash: FieldElement,
    pub executor_address: FieldElement,
    pub executor_class_hash: FieldElement,
    pub models: HashMap<String, ModelMetadata>,
}

impl WorldMetadata {
    /// Retrieves the metadata of a model.
    pub fn model(&self, name: impl AsRef<str>) -> Option<&ModelMetadata> {
        self.models.get(name.as_ref())
    }
}
