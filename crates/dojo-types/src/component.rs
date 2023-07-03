use serde::{Deserialize, Serialize};

/// Represents a component member.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    /// Name of the member.
    pub name: String,
    /// Type of the member.
    #[serde(rename = "type")]
    pub ty: String,
    pub slot: u64,
    pub offset: u8,
}
