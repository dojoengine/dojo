use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;

/// Represents a system's component dependency.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dependency {
    /// Name of the component.
    pub name: String,
    pub read: bool,
    pub write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetadata {
    pub name: String,
    pub class_hash: FieldElement,
}
