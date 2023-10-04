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
    pub components: HashMap<String, ModelMetadata>,
}
