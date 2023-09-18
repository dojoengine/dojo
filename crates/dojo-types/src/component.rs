use serde::{Deserialize, Serialize};

/// Represents a component member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    /// Name of the member.
    pub name: String,
    /// Type of the member.
    #[serde(rename = "type")]
    pub ty: String,
    pub key: bool,
}

impl std::fmt::Display for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Member {{ name: {}, type: {}, key: {} }}", self.name, self.ty, self.key)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum MemberType {
    Simple(String),
    Complex(Vec<Member>),
    Enum(Vec<String>),
}

impl std::fmt::Display for MemberType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MemberType::Simple(s) => write!(f, "{}", s),
            MemberType::Complex(members) => {
                let members = members.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", ");
                write!(f, "[{}]", members)
            }
            MemberType::Enum(values) => {
                let values = values.join(", ");
                write!(f, "Enum({})", values)
            }
        }
    }
}
