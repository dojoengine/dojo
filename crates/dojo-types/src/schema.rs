use std::any::type_name;

use cainome::cairo_serde::{ByteArray, CairoSerde};
use itertools::Itertools;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
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
    pub fn serialize(&self) -> Result<Vec<Felt>, PrimitiveError> {
        self.ty.serialize()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub schema: Ty,
    pub namespace: String,
    pub name: String,
    pub packed_size: u32,
    pub unpacked_size: u32,
    pub class_hash: Felt,
    pub contract_address: Felt,
    pub layout: Vec<Felt>,
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
    Array(Vec<Ty>),
    ByteArray(String),
}

impl Ty {
    pub fn name(&self) -> String {
        match self {
            Ty::Primitive(c) => c.to_string(),
            Ty::Struct(s) => s.name.clone(),
            Ty::Enum(e) => e.name.clone(),
            Ty::Tuple(tys) => format!("({})", tys.iter().map(|ty| ty.name()).join(", ")),
            Ty::Array(ty) => {
                if let Some(inner) = ty.first() {
                    format!("Array<{}>", inner.name())
                } else {
                    "Array".to_string()
                }
            }
            Ty::ByteArray(_) => "ByteArray".to_string(),
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

    /// If the `Ty` is an array, returns the associated [`Vec<Ty>`]. Returns `None` otherwise.
    pub fn as_array(&self) -> Option<&Vec<Ty>> {
        match self {
            Ty::Array(tys) => Some(tys),
            _ => None,
        }
    }

    /// If the `Ty` is a byte array, returns the associated [`String`]. Returns `None` otherwise.
    pub fn as_byte_array(&self) -> Option<&String> {
        match self {
            Ty::ByteArray(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn serialize(&self) -> Result<Vec<Felt>, PrimitiveError> {
        let mut felts = vec![];

        fn serialize_inner(ty: &Ty, felts: &mut Vec<Felt>) -> Result<(), PrimitiveError> {
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
                        .map(|v| Ok(vec![Felt::from(v)]))
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
                Ty::Array(items_ty) => {
                    let _ = serialize_inner(
                        &Ty::Primitive(Primitive::U32(Some(items_ty.len().try_into().unwrap()))),
                        felts,
                    );
                    for item_ty in items_ty {
                        serialize_inner(item_ty, felts)?;
                    }
                }
                Ty::ByteArray(bytes) => {
                    let bytearray = ByteArray::from_string(bytes)?;

                    felts.extend(ByteArray::cairo_serialize(&bytearray))
                }
            }
            Ok(())
        }

        serialize_inner(self, &mut felts)?;

        Ok(felts)
    }

    pub fn deserialize(&mut self, felts: &mut Vec<Felt>) -> Result<(), PrimitiveError> {
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
                let value = felts.remove(0);
                e.option = Some(value.to_u8().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u8>(),
                    value,
                })?);

                match &e.options[e.option.unwrap() as usize].ty {
                    // Skip deserializing the enum option if it has no type - unit type
                    Ty::Tuple(tuple) if tuple.is_empty() => {}
                    _ => {
                        e.options[e.option.unwrap() as usize].ty.deserialize(felts)?;
                    }
                }
            }
            Ty::Tuple(tys) => {
                for ty in tys {
                    ty.deserialize(felts)?;
                }
            }
            Ty::Array(items_ty) => {
                let value = felts.remove(0);
                let arr_len: u32 = value.to_u32().ok_or_else(|| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<u32>(), value }
                })?;

                let item_ty = items_ty.pop().unwrap();
                for _ in 0..arr_len {
                    let mut cur_item_ty = item_ty.clone();
                    cur_item_ty.deserialize(felts)?;
                    items_ty.push(cur_item_ty);
                }
            }
            Ty::ByteArray(bytes) => {
                let bytearray = ByteArray::cairo_deserialize(felts, 0)?;
                felts.drain(0..ByteArray::cairo_serialized_size(&bytearray));

                *bytes = ByteArray::to_string(&bytearray)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
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
                    Some(format!("tuple({})", tuple.iter().map(|ty| ty.name()).join(", ")))
                }
                Ty::Array(items_ty) => Some(format!("Array<{}>", items_ty[0].name())),
                Ty::ByteArray(_) => Some("ByteArray".to_string()),
                _ => None,
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
            Primitive::I8(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::I16(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::I32(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::I64(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
            Primitive::I128(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {}", value));
                }
            }
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

#[cfg(test)]
mod tests {
    use crypto_bigint::U256;
    use starknet::core::types::Felt;

    use super::*;
    use crate::primitive::Primitive;

    #[test]
    fn test_format_member() {
        let test_cases = vec![
            (
                Member {
                    name: "i8_field".to_string(),
                    ty: Ty::Primitive(Primitive::I8(Some(-42))),
                    key: false,
                },
                "  i8_field: i8 = -42",
            ),
            (
                Member {
                    name: "i16_field".to_string(),
                    ty: Ty::Primitive(Primitive::I16(Some(-1000))),
                    key: false,
                },
                "  i16_field: i16 = -1000",
            ),
            (
                Member {
                    name: "i32_field".to_string(),
                    ty: Ty::Primitive(Primitive::I32(Some(-100000))),
                    key: false,
                },
                "  i32_field: i32 = -100000",
            ),
            (
                Member {
                    name: "i64_field".to_string(),
                    ty: Ty::Primitive(Primitive::I64(Some(-1000000000))),
                    key: false,
                },
                "  i64_field: i64 = -1000000000",
            ),
            (
                Member {
                    name: "i128_field".to_string(),
                    ty: Ty::Primitive(Primitive::I128(Some(-1000000000000000000))),
                    key: false,
                },
                "  i128_field: i128 = -1000000000000000000",
            ),
            (
                Member {
                    name: "u8_field".to_string(),
                    ty: Ty::Primitive(Primitive::U8(Some(255))),
                    key: false,
                },
                "  u8_field: u8 = 255",
            ),
            (
                Member {
                    name: "u16_field".to_string(),
                    ty: Ty::Primitive(Primitive::U16(Some(65535))),
                    key: false,
                },
                "  u16_field: u16 = 65535",
            ),
            (
                Member {
                    name: "u32_field".to_string(),
                    ty: Ty::Primitive(Primitive::U32(Some(4294967295))),
                    key: false,
                },
                "  u32_field: u32 = 4294967295",
            ),
            (
                Member {
                    name: "u64_field".to_string(),
                    ty: Ty::Primitive(Primitive::U64(Some(18446744073709551615))),
                    key: false,
                },
                "  u64_field: u64 = 18446744073709551615",
            ),
            (
                Member {
                    name: "u128_field".to_string(),
                    ty: Ty::Primitive(Primitive::U128(Some(
                        340282366920938463463374607431768211455,
                    ))),
                    key: false,
                },
                "  u128_field: u128 = 340282366920938463463374607431768211455",
            ),
            (
                Member {
                    name: "u256_field".to_string(),
                    ty: Ty::Primitive(Primitive::U256(Some(U256::from_u128(123456789_u128)))),
                    key: false,
                },
                "  u256_field: u256 = \
                 00000000000000000000000000000000000000000000000000000000075BCD15",
            ),
            (
                Member {
                    name: "bool_field".to_string(),
                    ty: Ty::Primitive(Primitive::Bool(Some(true))),
                    key: false,
                },
                "  bool_field: bool = true",
            ),
            (
                Member {
                    name: "felt252_field".to_string(),
                    ty: Ty::Primitive(Primitive::Felt252(Some(
                        Felt::from_hex("0x123abc").unwrap(),
                    ))),
                    key: false,
                },
                "  felt252_field: felt252 = 0x123abc",
            ),
            (
                Member {
                    name: "enum_field".to_string(),
                    ty: Ty::Enum(Enum {
                        name: "TestEnum".to_string(),
                        option: Some(1),
                        options: vec![
                            EnumOption { name: "OptionA".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "OptionB".to_string(), ty: Ty::Tuple(vec![]) },
                        ],
                    }),
                    key: false,
                },
                "  enum_field: TestEnum = OptionB",
            ),
        ];

        for (member, expected) in test_cases {
            assert_eq!(format_member(&member), expected);
        }
    }
}
