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
        if self.key {
            write!(f, "#[key]\n{}: {}", self.name, self.ty)
        } else {
            write!(f, "{}: {}", self.name, self.ty)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Ty {
    Simple(String),
    Struct(Struct),
    Enum(Enum),
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Simple(s) => write!(f, "{}", s),
            Ty::Struct(structure) => {
                let members =
                    structure.children.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", ");
                write!(f, "{}", members)
            }
            Ty::Enum(e) => {
                let values = e.values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ");
                write!(f, "{}", values)
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Struct {
    pub name: String,
    pub children: Vec<Member>,
}

impl std::fmt::Display for Struct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        for child in &self.children {
            write!(f, "\n{}", child)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Enum {
    pub name: String,
    pub values: Vec<Ty>,
}

impl std::fmt::Display for Enum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        for value in &self.values {
            write!(f, "\n{}", value)?;
        }
        Ok(())
    }
}
