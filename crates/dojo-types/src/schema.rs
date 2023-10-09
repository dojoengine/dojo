use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use starknet::core::types::FieldElement;
use strum_macros::AsRefStr;

use crate::primitive::{Primitive, PrimitiveError};

/// Represents a model member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    pub name: String,
    pub ty: Ty,
    pub key: bool,
}

impl Member {
    pub fn serialize(&self) -> Result<Vec<FieldElement>, PrimitiveError> {
        self.ty.serialize()
    }
}

/// Represents a model of an entity
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityModel {
    pub model: String,
    pub keys: Vec<FieldElement>,
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
#[derive(AsRefStr, Clone, Debug, Serialize, Deserialize, PartialEq)]
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

                    for (_, child) in &e.options {
                        serialize_inner(child, felts)?;
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
                for (_, child) in &mut e.options {
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
                for child in &e.options {
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
                    for child in &e.options {
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

impl Struct {
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Enum {
    pub name: String,
    pub option: Option<u8>,
    pub options: Vec<(String, Ty)>,
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

        Ok(self.options[option].0.clone())
    }

    pub fn to_sql_value(&self) -> Result<String, EnumError> {
        Ok(format!("'{}'", self.option()?))
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

pub fn to_value(ty: &Ty) -> Value {
    match ty {
        Ty::Primitive(primitive) => return primitive_value_json(*primitive),

        Ty::Struct(struct_ty) => {
            let mut json_map = serde_json::Map::new();
            for child in &struct_ty.children {
                json_map.insert(child.name.to_owned(), to_value(&child.ty));
            }
            return Value::Object(json_map);
        }

        Ty::Enum(_) => unimplemented!("enum not supported"),
        Ty::Tuple(_) => unimplemented!("tuple not supported"),
    }
}

fn primitive_value_json(primitive: Primitive) -> Value {
    match primitive {
        Primitive::Bool(Some(value)) => Value::Bool(value),
        Primitive::U8(Some(value)) => Value::Number(value.into()),
        Primitive::U16(Some(value)) => Value::Number(value.into()),
        Primitive::U32(Some(value)) => Value::Number(value.into()),
        Primitive::U64(Some(value)) => Value::Number(value.into()),
        Primitive::U128(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::U256(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::USize(Some(value)) => Value::Number(value.into()),
        Primitive::ContractAddress(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::ClassHash(Some(value)) => Value::String(format!("{value:#x}")),
        Primitive::Felt252(Some(value)) => Value::String(format!("{value:#x}")),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod test {

    use serde_json::json;
    use starknet::macros::felt;

    use super::*;

    #[test]
    fn parse_ty_to_value() {
        let expected_ty = Ty::Struct(Struct {
            name: "Position".into(),
            children: vec![
                Member {
                    name: "player".into(),
                    key: false,
                    ty: Ty::Primitive(Primitive::ContractAddress(Some(felt!("0x123")))),
                },
                Member {
                    name: "is_dead".into(),
                    key: false,
                    ty: Ty::Primitive(Primitive::Bool(Some(true))),
                },
                Member {
                    name: "points".into(),
                    key: false,
                    ty: Ty::Primitive(Primitive::U32(Some(200))),
                },
                Member {
                    name: "vec".into(),
                    key: false,
                    ty: Ty::Struct(Struct {
                        name: "vec".into(),
                        children: vec![
                            Member {
                                name: "x".into(),
                                key: false,
                                ty: Ty::Primitive(Primitive::U128(Some(10))),
                            },
                            Member {
                                name: "y".into(),
                                key: false,
                                ty: Ty::Primitive(Primitive::U128(Some(10))),
                            },
                        ],
                    }),
                },
            ],
        });

        let expected_json = json!({
            "player": "0x123",
            "is_dead": true,
            "points": 200,
            "vec": {
                "x": "0xa",
                "y": "0xa",
            },
        });

        let actual_json = to_value(&expected_ty);
        assert_eq!(expected_json, actual_json)
    }
}
