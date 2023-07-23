use serde::{Deserialize, Serialize};

/// Represents a system's component dependency.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dependency {
    /// Name of the component.
    pub name: String,
    pub read: bool,
    pub write: bool,
}
