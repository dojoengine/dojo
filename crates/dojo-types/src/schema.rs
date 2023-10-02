use itertools::Itertools;
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;

use crate::core::{CairoType, CairoTypeError};

/// Represents a model member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    pub name: String,
    pub ty: Ty,
    pub key: bool,
}

/// Represents a model of an entity
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityModel {
    pub model: String,
    pub keys: Vec<FieldElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub name: String,
    pub size: u32,
    pub class_hash: FieldElement,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Ty {
    Primitive(CairoType),
    Struct(Struct),
    Enum(Enum),
    Tuple(Vec<Ty>),
}

impl Ty {
    pub fn name(&self) -> String {
        match self {
            Ty::Primitive(c) => c.to_string(),
            Ty::Struct(s) => s.name.clone(),
            Ty::Enum(e) => e.name.clone(),
            Ty::Tuple(tys) => format!("({})", tys.iter().map(|ty| ty.name()).join(", ")),
        }
    }

    pub fn iter(&self) -> TyIter<'_> {
        TyIter { stack: vec![self] }
    }

    pub fn deserialize(&mut self, felts: &mut Vec<FieldElement>) -> Result<(), CairoTypeError> {
        match self {
            Ty::Primitive(c) => {
                c.set_value_from_felts(felts)?;
            }
            Ty::Struct(s) => {
                for child in &mut s.children {
                    child.ty.deserialize(felts)?;
                }
            }
            Ty::Enum(e) => {
                for (_, child) in &mut e.children {
                    child.deserialize(felts)?;
                }
            }
            Ty::Tuple(tys) => {
                for ty in tys {
                    ty.deserialize(felts)?;
                }
            }
        }
        Ok(())
    }
}

pub struct TyIter<'a> {
    stack: Vec<&'a Ty>,
}

impl<'a> Iterator for TyIter<'a> {
    type Item = &'a Ty;

    fn next(&mut self) -> Option<Self::Item> {
        let ty = self.stack.pop()?;
        match ty {
            Ty::Struct(s) => {
                for child in &s.children {
                    self.stack.push(&child.ty);
                }
            }
            Ty::Enum(e) => {
                for child in &e.children {
                    self.stack.push(&child.1);
                }
            }
            _ => {}
        }
        Some(ty)
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = self
            .iter()
            .filter_map(|ty| match ty {
                Ty::Primitive(_) => None,
                Ty::Struct(s) => {
                    let mut struct_str = format!("struct {} {{\n", s.name);
                    for member in &s.children {
                        struct_str.push_str(&format!("{},\n", format_member(member)));
                    }
                    struct_str.push('}');
                    Some(struct_str)
                }
                Ty::Enum(e) => {
                    let mut enum_str = format!("enum {} {{\n", e.name);
                    for child in &e.children {
                        enum_str.push_str(&format!("  {}\n", child.0));
                    }
                    enum_str.push('}');
                    Some(enum_str)
                }
                Ty::Tuple(tuple) => {
                    if tuple.is_empty() {
                        None
                    } else {
                        Some(ty.name())
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        write!(f, "{}", str)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Struct {
    pub name: String,
    pub children: Vec<Member>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Enum {
    pub name: String,
    pub children: Vec<(String, Ty)>,
}

fn format_member(m: &Member) -> String {
    let mut str = if m.key {
        format!("  #[key]\n  {}: {}", m.name, m.ty.name())
    } else {
        format!("  {}: {}", m.name, m.ty.name())
    };

    if let Ty::Primitive(ty) = &m.ty {
        match ty {
            CairoType::U8(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            CairoType::U16(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            CairoType::U32(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            CairoType::U64(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            CairoType::U128(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            CairoType::U256(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            CairoType::USize(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            CairoType::Bool(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            CairoType::Felt252(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {:#x}", value));
                }
            }
            CairoType::ClassHash(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {:#x}", value));
                }
            }
            CairoType::ContractAddress(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {:#x}", value));
                }
            }
        }
    }

    str
}
