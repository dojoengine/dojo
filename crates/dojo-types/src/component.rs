use serde::{Deserialize, Serialize};

/// Represents a component member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    pub name: String,
    pub ty: Ty,
    pub key: bool,
}

impl std::fmt::Display for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Member {{ name: {}, type: {}, key: {} }}", self.name, self.ty, self.key)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Ty {
    Simple(String),
    Struct(Struct),
    Enum(Enum),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Struct {
    pub name: String,
    pub children: Vec<Member>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Enum {
    pub name: String,
    pub values: Vec<Ty>,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Simple(s) => write!(f, "{}", s),
            Ty::Struct(members) => {
                let members =
                    members.children.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", ");
                write!(f, "[{}]", members)
            }
            Ty::Enum(e) => {
                // let values = e.values.join(", ");
                let values = "";
                write!(f, "Enum({})", values)
            }
        }
    }
}
