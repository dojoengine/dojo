use itertools::Itertools;
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use strum_macros::AsRefStr;

use crate::primitive::{Primitive, PrimitiveError};

/// Represents a model member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub struct Member {
    pub name: String,
    #[serde(rename = "member_type")]
    pub ty: Ty,
    pub key: bool,
}

impl Member {
    pub fn serialize(&self) -> Result<Vec<FieldElement>, PrimitiveError> {
        self.ty.serialize()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub schema: Ty,
    pub name: String,
    pub packed_size: u32,
    pub unpacked_size: u32,
    pub class_hash: FieldElement,
    pub layout: Vec<FieldElement>,
}

/// Represents all possible types in Cairo
#[derive(AsRefStr, Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
#[serde(tag = "type", content = "content")]
#[serde(rename_all = "lowercase")]
pub enum Ty {
    Primitive(Primitive),
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

    /// If the `Ty` is a primitive, returns the associated [`Primitive`]. Returns `None`
    /// otherwise.
    pub fn as_primitive(&self) -> Option<&Primitive> {
        match self {
            Ty::Primitive(c) => Some(c),
            _ => None,
        }
    }

    /// If the `Ty` is a struct, returns the associated [`Struct`]. Returns `None` otherwise.
    pub fn as_struct(&self) -> Option<&Struct> {
        match self {
            Ty::Struct(s) => Some(s),
            _ => None,
        }
    }

    /// If the `Ty` is an enum, returns the associated [`Enum`]. Returns `None` otherwise.
    pub fn as_enum(&self) -> Option<&Enum> {
        match self {
            Ty::Enum(e) => Some(e),
            _ => None,
        }
    }

    /// If the `Ty` is a tuple, returns the associated [`Vec<Ty>`]. Returns `None` otherwise.
    pub fn as_tuple(&self) -> Option<&Vec<Ty>> {
        match self {
            Ty::Tuple(tys) => Some(tys),
            _ => None,
        }
    }

    pub fn serialize(&self) -> Result<Vec<FieldElement>, PrimitiveError> {
        let mut felts = vec![];

        fn serialize_inner(ty: &Ty, felts: &mut Vec<FieldElement>) -> Result<(), PrimitiveError> {
            match ty {
                Ty::Primitive(c) => {
                    felts.extend(c.serialize()?);
                }
                Ty::Struct(s) => {
                    for child in &s.children {
                        serialize_inner(&child.ty, felts)?;
                    }
                }
                Ty::Enum(e) => {
                    let option = e
                        .option
                        .map(|v| Ok(vec![FieldElement::from(v)]))
                        .unwrap_or(Err(PrimitiveError::MissingFieldElement))?;
                    felts.extend(option);

                    for EnumOption { ty, .. } in &e.options {
                        serialize_inner(ty, felts)?;
                    }
                }
                Ty::Tuple(tys) => {
                    for ty in tys {
                        serialize_inner(ty, felts)?;
                    }
                }
            }
            Ok(())
        }

        serialize_inner(self, &mut felts)?;

        Ok(felts)
    }

    pub fn deserialize(&mut self, felts: &mut Vec<FieldElement>) -> Result<(), PrimitiveError> {
        match self {
            Ty::Primitive(c) => {
                c.deserialize(felts)?;
            }
            Ty::Struct(s) => {
                for child in &mut s.children {
                    child.ty.deserialize(felts)?;
                }
            }
            Ty::Enum(e) => {
                e.option =
                    Some(felts.remove(0).try_into().map_err(PrimitiveError::ValueOutOfRange)?);
                for EnumOption { ty, .. } in &mut e.options {
                    ty.deserialize(felts)?;
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
                for child in &e.options {
                    self.stack.push(&child.ty);
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
                    for child in &e.options {
                        enum_str.push_str(&format!("  {}\n", child.name));
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub struct Struct {
    pub name: String,
    pub children: Vec<Member>,
}

impl Struct {
    /// Returns the struct member with the given name. Returns `None` if no such member exists.
    pub fn get(&self, field: &str) -> Option<&Ty> {
        self.children.iter().find(|m| m.name == field).map(|m| &m.ty)
    }

    pub fn keys(&self) -> Vec<Member> {
        self.children.iter().filter(|m| m.key).cloned().collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EnumError {
    #[error("Enum option not set")]
    OptionNotSet,
    #[error("Enum option invalid")]
    OptionInvalid,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub struct Enum {
    pub name: String,
    pub option: Option<u8>,
    pub options: Vec<EnumOption>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash, Eq)]
pub struct EnumOption {
    pub name: String,
    pub ty: Ty,
}

impl Enum {
    pub fn option(&self) -> Result<String, EnumError> {
        let option: usize = if let Some(option) = self.option {
            option as usize
        } else {
            return Err(EnumError::OptionNotSet);
        };

        if option >= self.options.len() {
            return Err(EnumError::OptionInvalid);
        }

        Ok(self.options[option].name.clone())
    }

    pub fn set_option(&mut self, name: &str) -> Result<(), EnumError> {
        match self.options.iter().position(|option| option.name == name) {
            Some(index) => {
                self.option = Some(index as u8);
                Ok(())
            }
            None => Err(EnumError::OptionInvalid),
        }
    }

    pub fn to_sql_value(&self) -> Result<String, EnumError> {
        self.option()
    }
}

fn format_member(m: &Member) -> String {
    let mut str = if m.key {
        format!("  #[key]\n  {}: {}", m.name, m.ty.name())
    } else {
        format!("  {}: {}", m.name, m.ty.name())
    };

    if let Ty::Primitive(ty) = &m.ty {
        match ty {
            Primitive::U8(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::U16(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::U32(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::U64(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::U128(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::U256(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::USize(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::Bool(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::Felt252(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {:#x}", value));
                }
            }
            Primitive::ClassHash(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {:#x}", value));
                }
            }
            Primitive::ContractAddress(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {:#x}", value));
                }
            }
        }
    } else if let Ty::Enum(e) = &m.ty {
        match e.option() {
            Ok(option) => str.push_str(&format!(" = {option}")),
            Err(_) => str.push_str(" = Invalid Option"),
        }
    }

    str
}
