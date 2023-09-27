use serde::{Deserialize, Serialize};

/// Represents a system's model dependency.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dependency {
    /// Name of the model.
    pub name: String,
    pub read: bool,
    pub write: bool,
}
