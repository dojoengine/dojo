use std::any::type_name;

use cainome::cairo_serde::{ByteArray, CairoSerde};
use indexmap::IndexMap;
use itertools::Itertools;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
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
    FixedSizeArray((Vec<Ty>, u32)),
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
            Ty::FixedSizeArray((ty, size)) => {
                if let Some(ty) = ty.first() {
                    format!("[{}; {}]", ty.name(), size)
                } else {
                    "[; 0]".to_string()
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

    /// If the `Ty` is a fixed size array, returns the associated [`Vec<Ty>`]. Returns `None`
    /// otherwise.
    pub fn as_fixed_size_array(&self) -> Option<&(Vec<Ty>, u32)> {
        match self {
            Ty::FixedSizeArray(tys) => Some(tys),
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

                    // TODO: we should increment `option` is the model does not use the legacy
                    // storage system. But is this `serialize` function still
                    // used ?

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
                Ty::FixedSizeArray((items_ty, size)) => {
                    let item_ty = &items_ty[0];
                    for _ in 0..*size {
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

    pub fn deserialize(
        &mut self,
        felts: &mut Vec<Felt>,
        legacy_storage: bool,
    ) -> Result<(), PrimitiveError> {
        if felts.is_empty() {
            // return early if there are no felts to deserialize
            return Ok(());
        }

        match self {
            Ty::Primitive(c) => {
                c.deserialize(felts)?;
            }
            Ty::Struct(s) => {
                for child in &mut s.children {
                    child.ty.deserialize(felts, legacy_storage)?;
                }
            }
            Ty::Enum(e) => {
                let value = felts.remove(0);
                let actual_selector = value.to_u8().ok_or_else(|| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<u8>(), value }
                })?;

                let mut selector = actual_selector;

                // Th new `DojoStore`` trait, enum variants indices start from 1. The 0 value is
                // reserved for uninitialized enum.
                if !legacy_storage {
                    if selector == 0 {
                        // We set to None here in case this is not the first time we deserialize
                        // `self`. In which case, previous deserialization might have set the option
                        // to Some.
                        e.option = None;
                        return Ok(());
                    } else {
                        // With the new storage system using `DojoStore` trait, variant indices
                        // start from 1.
                        selector -= 1;
                    }
                }

                e.option = Some(selector);

                let selected_opt = e
                    .options
                    .get_mut(selector as usize)
                    .ok_or_else(|| PrimitiveError::InvalidEnumSelector { actual_selector })?;

                // No further deserialization needed if the enum variant is a unit type
                if let Ty::Tuple(tuple) = &selected_opt.ty {
                    if tuple.is_empty() {
                        return Ok(());
                    }
                }

                selected_opt.ty.deserialize(felts, legacy_storage)?;
            }
            Ty::Tuple(tys) => {
                for ty in tys {
                    ty.deserialize(felts, legacy_storage)?;
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
                    cur_item_ty.deserialize(felts, legacy_storage)?;
                    items_ty.push(cur_item_ty);
                }
            }
            Ty::FixedSizeArray((items_ty, size)) => {
                debug_assert_eq!(items_ty.len(), *size as usize);
                for elem in items_ty {
                    elem.deserialize(felts, legacy_storage)?;
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

    /// Returns a new Ty containing only the differences between self and other
    pub fn diff(&self, other: &Ty) -> Option<Ty> {
        match (self, other) {
            (Ty::Struct(s1), Ty::Struct(s2)) => {
                // Find members that exist in s1 but not in s2, or are different
                let diff_children: Vec<Member> = s1
                    .children
                    .iter()
                    .filter_map(|m1| {
                        if let Some(m2) = s2.children.iter().find(|m2| m2.name == m1.name) {
                            // Member exists in both - check if types are different
                            m1.ty.diff(&m2.ty).map(|diff_ty| Member {
                                name: m1.name.clone(),
                                ty: diff_ty,
                                key: m1.key,
                            })
                        } else {
                            // Member doesn't exist in s2
                            Some(m1.clone())
                        }
                    })
                    .collect();

                if diff_children.is_empty() {
                    None
                } else {
                    Some(Ty::Struct(Struct { name: s1.name.clone(), children: diff_children }))
                }
            }
            (Ty::Enum(e1), Ty::Enum(e2)) => {
                // Find options that exist in e1 but not in e2, or are different
                let diff_options: Vec<EnumOption> = e1
                    .options
                    .iter()
                    .filter_map(|o1| {
                        if let Some(o2) = e2.options.iter().find(|o2| o2.name == o1.name) {
                            // Option exists in both - check if types are different
                            o1.ty
                                .diff(&o2.ty)
                                .map(|diff_ty| EnumOption { name: o1.name.clone(), ty: diff_ty })
                        } else {
                            // Option doesn't exist in e2
                            Some(o1.clone())
                        }
                    })
                    .collect();

                if diff_options.is_empty() {
                    None
                } else {
                    Some(Ty::Enum(Enum {
                        name: e1.name.clone(),
                        option: e1.option,
                        options: diff_options,
                    }))
                }
            }
            (Ty::Tuple(t1), Ty::Tuple(t2)) => {
                if t1.len() != t2.len() {
                    Some(Ty::Tuple(
                        t1.iter()
                            .filter_map(|ty| if !t2.contains(ty) { Some(ty.clone()) } else { None })
                            .collect(),
                    ))
                } else {
                    // Compare each tuple element recursively
                    let diff_elements: Vec<Ty> =
                        t1.iter().zip(t2.iter()).filter_map(|(ty1, ty2)| ty1.diff(ty2)).collect();

                    if diff_elements.is_empty() { None } else { Some(Ty::Tuple(diff_elements)) }
                }
            }
            (Ty::Array(a1), Ty::Array(a2)) => {
                if a1 == a2 {
                    None
                } else {
                    Some(Ty::Array(a1.clone()))
                }
            }
            (Ty::FixedSizeArray(a1), Ty::FixedSizeArray(a2)) => {
                if a1 == a2 {
                    None
                } else {
                    Some(Ty::FixedSizeArray(a1.clone()))
                }
            }
            (Ty::ByteArray(b1), Ty::ByteArray(b2)) => {
                if b1 == b2 {
                    None
                } else {
                    Some(Ty::ByteArray(b1.clone()))
                }
            }
            (Ty::Primitive(p1), Ty::Primitive(p2)) => {
                if p1 == p2 {
                    None
                } else {
                    Some(Ty::Primitive(*p1))
                }
            }
            // Different types entirely - we cannot diff them
            _ => {
                panic!("Type mismatch between self {:?} and other {:?}", self.name(), other.name())
            }
        }
    }

    /// Convert a Ty to a JSON Value
    pub fn to_json_value(&self) -> Result<JsonValue, PrimitiveError> {
        match self {
            Ty::Primitive(primitive) => primitive.to_json_value(),
            Ty::Struct(s) => {
                let mut obj = IndexMap::new();
                for member in &s.children {
                    obj.insert(member.name.clone(), member.ty.to_json_value()?);
                }
                Ok(json!(obj))
            }
            Ty::Enum(e) => {
                let option = e.option().map_err(|_| PrimitiveError::MissingFieldElement)?;
                Ok(json!({
                    option.name.clone(): option.ty.to_json_value()?
                }))
            }
            Ty::Array(items) | Ty::Tuple(items) | Ty::FixedSizeArray((items, _)) => {
                let values: Result<Vec<_>, _> = items.iter().map(|ty| ty.to_json_value()).collect();
                Ok(json!(values?))
            }
            Ty::ByteArray(bytes) => Ok(json!(bytes.clone())),
        }
    }

    /// Parse a JSON Value into a Ty
    pub fn from_json_value(&mut self, value: JsonValue) -> Result<(), PrimitiveError> {
        match (self, value) {
            (Ty::Primitive(primitive), value) => {
                primitive.from_json_value(value)?;
            }
            (Ty::Struct(s), JsonValue::Object(obj)) => {
                for member in &mut s.children {
                    if let Some(value) = obj.get(&member.name) {
                        member.ty.from_json_value(value.clone())?;
                    }
                }
            }
            (Ty::Enum(e), JsonValue::Object(obj)) => {
                if let Some((name, value)) = obj.into_iter().next() {
                    e.set_option(&name).map_err(|_| PrimitiveError::TypeMismatch)?;
                    if let Some(option) = e.option {
                        e.options[option as usize].ty.from_json_value(value)?;
                    }
                }
            }
            (Ty::Array(items), JsonValue::Array(values)) => {
                if values.is_empty() {
                    items.clear();
                } else if items.is_empty() {
                    return Err(PrimitiveError::TypeMismatch);
                } else {
                    let template = items[0].clone();
                    items.clear();
                    for value in values {
                        let mut item = template.clone();
                        item.from_json_value(value)?;
                        items.push(item);
                    }
                }
            }
            (Ty::FixedSizeArray((items, size)), JsonValue::Array(values)) => {
                if values.len() != *size as usize {
                    return Err(PrimitiveError::TypeMismatch);
                }
                if values.is_empty() {
                    items.clear();
                } else if items.is_empty() {
                    return Err(PrimitiveError::TypeMismatch);
                } else {
                    let template = items[0].clone();
                    items.clear();
                    for value in values {
                        let mut item = template.clone();
                        item.from_json_value(value)?;
                        items.push(item);
                    }
                }
            }
            (Ty::Tuple(items), JsonValue::Array(values)) => {
                if items.len() != values.len() {
                    return Err(PrimitiveError::TypeMismatch);
                }
                for (item, value) in items.iter_mut().zip(values) {
                    item.from_json_value(value)?;
                }
            }
            (Ty::ByteArray(bytes), JsonValue::String(s)) => {
                *bytes = s;
            }
            _ => return Err(PrimitiveError::TypeMismatch),
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
                Ty::FixedSizeArray((items_ty, length)) => {
                    let item_ty = &items_ty[0];
                    Some(format!("[{}; {}]", item_ty.name(), *length))
                }
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
    pub fn option(&self) -> Result<&EnumOption, EnumError> {
        let option: usize = if let Some(option) = self.option {
            option as usize
        } else {
            return Err(EnumError::OptionNotSet);
        };

        if option >= self.options.len() {
            return Err(EnumError::OptionInvalid);
        }

        Ok(&self.options[option])
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

    pub fn to_sql_value(&self) -> String {
        self.option().unwrap_or(&self.options[0]).name.clone()
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
            Primitive::EthAddress(value) => {
                if let Some(value) = value {
                    str.push_str(&format!(" = {:#x}", value));
                }
            }
        }
    } else if let Ty::Enum(e) = &m.ty {
        match e.option() {
            Ok(option) => str.push_str(&format!(" = {}", option.name)),
            Err(_) => str.push_str(" = Invalid Option"),
        }
    }

    str
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use crypto_bigint::U256;
    use num_traits::FromPrimitive;
    use starknet::core::types::Felt;
    use starknet::macros::felt;

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

    #[test]
    fn test_ty_diff() {
        // Test struct diff
        let struct1 = Ty::Struct(Struct {
            name: "TestStruct".to_string(),
            children: vec![
                Member {
                    name: "field1".to_string(),
                    ty: Ty::Primitive(Primitive::U32(None)),
                    key: false,
                },
                Member {
                    name: "field2".to_string(),
                    ty: Ty::Primitive(Primitive::U32(None)),
                    key: false,
                },
                Member {
                    name: "field3".to_string(),
                    ty: Ty::Primitive(Primitive::U32(None)),
                    key: false,
                },
            ],
        });

        let struct2 = Ty::Struct(Struct {
            name: "TestStruct".to_string(),
            children: vec![Member {
                name: "field1".to_string(),
                ty: Ty::Primitive(Primitive::U32(None)),
                key: false,
            }],
        });

        // Should show only field2 and field3 as differences
        let diff = struct1.diff(&struct2).unwrap();
        if let Ty::Struct(s) = diff {
            assert_eq!(s.children.len(), 2);
            assert_eq!(s.children[0].name, "field2");
            assert_eq!(s.children[1].name, "field3");
        } else {
            panic!("Expected Struct diff");
        }

        // Test enum diff
        let enum1 = Ty::Enum(Enum {
            name: "TestEnum".to_string(),
            option: None,
            options: vec![
                EnumOption { name: "Option1".to_string(), ty: Ty::Tuple(vec![]) },
                EnumOption { name: "Option2".to_string(), ty: Ty::Tuple(vec![]) },
            ],
        });

        let enum2 = Ty::Enum(Enum {
            name: "TestEnum".to_string(),
            option: None,
            options: vec![EnumOption { name: "Option1".to_string(), ty: Ty::Tuple(vec![]) }],
        });

        // Should show only Option2 as difference
        let diff = enum1.diff(&enum2).unwrap();
        if let Ty::Enum(e) = diff {
            assert_eq!(e.options.len(), 1);
            assert_eq!(e.options[0].name, "Option2");
        } else {
            panic!("Expected Enum diff");
        }

        // Test no differences
        let same_struct = struct2.diff(&struct2);
        assert!(same_struct.is_none());
    }

    #[test]
    fn ty_deserialize_legacy_enum() {
        // enum Direction {
        //     Up,
        //     Bottom,
        //     Left,
        //     Right,
        // }

        let mut ty = Ty::Enum(Enum {
            name: "Direction".to_string(),
            option: None,
            options: vec![
                EnumOption { name: "Up".to_string(), ty: Ty::Tuple(Vec::new()) },
                EnumOption { name: "Bottom".to_string(), ty: Ty::Tuple(Vec::new()) },
                EnumOption { name: "Left".to_string(), ty: Ty::Tuple(Vec::new()) },
                EnumOption { name: "Right".to_string(), ty: Ty::Tuple(Vec::new()) },
            ],
        });

        for i in 0..4 {
            let mut felts = vec![Felt::from_i32(i).unwrap()];
            ty.deserialize(&mut felts, true).expect("failed to deserialize");
            assert!(felts.is_empty());
            assert_matches!(&ty, Ty::Enum(Enum {  option, .. }) => assert_eq!(option, &Some(i as u8)));
        }

        let mut felts = vec![felt!("0x4")];
        let result = ty.deserialize(&mut felts, true);
        assert!(felts.is_empty());
        assert_matches!(&result, Err(PrimitiveError::InvalidEnumSelector { actual_selector: 4 }));
    }

    #[test]
    fn ty_deserialize_enum() {
        // enum Direction {
        //     Up,
        //     Bottom,
        //     Left,
        //     Right,
        // }

        let mut ty = Ty::Enum(Enum {
            name: "Direction".to_string(),
            option: None,
            options: vec![
                EnumOption { name: "Up".to_string(), ty: Ty::Tuple(Vec::new()) },
                EnumOption { name: "Bottom".to_string(), ty: Ty::Tuple(Vec::new()) },
                EnumOption { name: "Left".to_string(), ty: Ty::Tuple(Vec::new()) },
                EnumOption { name: "Right".to_string(), ty: Ty::Tuple(Vec::new()) },
            ],
        });

        for i in 0..4 {
            let mut felts = vec![Felt::from_i32(i + 1).unwrap()]; // non legacy store enum indices starts from 1
            ty.deserialize(&mut felts, false).expect("failed to deserialize");
            assert!(felts.is_empty());
            assert_matches!(&ty, Ty::Enum(Enum {  option, .. }) => assert_eq!(option, &Some(i as u8)));
        }

        let mut felts = vec![felt!("0x5")];
        let result = ty.deserialize(&mut felts, false);
        assert!(felts.is_empty());
        assert_matches!(&result, Err(PrimitiveError::InvalidEnumSelector { actual_selector: 5 }));

        // deserializes from an uninitialized storage
        let mut felts = vec![felt!("0x0")];
        ty.deserialize(&mut felts, false).expect("failed to deserialize");
        assert!(felts.is_empty());
        assert_matches!(&ty, Ty::Enum(Enum { option: None, .. }));
    }

    #[test]
    fn test_to_json_value_comprehensive_with_round_trip() {
        let test_cases = vec![
            // Test Array
            Ty::Array(vec![
                Ty::Primitive(Primitive::U32(Some(1))),
                Ty::Primitive(Primitive::U32(Some(2))),
                Ty::Primitive(Primitive::U32(Some(3))),
            ]),
            // Test Tuple
            Ty::Tuple(vec![
                Ty::Primitive(Primitive::U32(Some(42))),
                Ty::Primitive(Primitive::Bool(Some(true))),
                Ty::ByteArray("hello".to_string()),
            ]),
            // Test FixedSizeArray
            Ty::FixedSizeArray((
                vec![
                    Ty::Primitive(Primitive::Felt252(Some(felt!("0x1")))),
                    Ty::Primitive(Primitive::Felt252(Some(felt!("0x2")))),
                    Ty::Primitive(Primitive::Felt252(Some(felt!("0x3")))),
                ],
                3,
            )),
            // Test nested structures
            Ty::Tuple(vec![
                Ty::Array(vec![
                    Ty::Primitive(Primitive::U8(Some(10))),
                    Ty::Primitive(Primitive::U8(Some(20))),
                ]),
                Ty::Tuple(vec![
                    Ty::Primitive(Primitive::Bool(Some(false))),
                    Ty::Primitive(Primitive::U16(Some(300))),
                ]),
            ]),
            // Test nested Array
            Ty::Array(vec![
                Ty::Tuple(vec![
                    Ty::Primitive(Primitive::U16(Some(100))),
                    Ty::Primitive(Primitive::Bool(Some(false))),
                ]),
                Ty::Tuple(vec![
                    Ty::Primitive(Primitive::U16(Some(200))),
                    Ty::Primitive(Primitive::Bool(Some(true))),
                ]),
            ]),
            // Test Struct
            Ty::Struct(Struct {
                name: "TestStruct".to_string(),
                children: vec![
                    Member {
                        name: "field1".to_string(),
                        ty: Ty::Primitive(Primitive::U32(Some(42))),
                        key: false,
                    },
                    Member {
                        name: "field2".to_string(),
                        ty: Ty::Primitive(Primitive::Bool(Some(true))),
                        key: false,
                    },
                    Member {
                        name: "nested_array".to_string(),
                        ty: Ty::Array(vec![
                            Ty::Primitive(Primitive::U8(Some(1))),
                            Ty::Primitive(Primitive::U8(Some(2))),
                        ]),
                        key: false,
                    },
                ],
            }),
            // Test Enum
            Ty::Enum(Enum {
                name: "TestEnum".to_string(),
                option: Some(1),
                options: vec![
                    EnumOption { name: "VariantA".to_string(), ty: Ty::Tuple(vec![]) },
                    EnumOption {
                        name: "VariantB".to_string(),
                        ty: Ty::Primitive(Primitive::U32(Some(123))),
                    },
                ],
            }),
            // Test another Enum
            Ty::Enum(Enum {
                name: "Status".to_string(),
                option: Some(0),
                options: vec![
                    EnumOption {
                        name: "Active".to_string(),
                        ty: Ty::Primitive(Primitive::U32(Some(100))),
                    },
                    EnumOption { name: "Inactive".to_string(), ty: Ty::Tuple(vec![]) },
                ],
            }),
            // Test ByteArray
            Ty::ByteArray("Hello, World!".to_string()),
            // Test empty collections
            Ty::Array(vec![]),
            Ty::Tuple(vec![]),
            Ty::FixedSizeArray((vec![], 0)),
        ];

        // Test specific expected JSON values for key types
        let array_json = test_cases[0].to_json_value().expect("failed to serialize array");
        let expected_array = json!([1, 2, 3]);
        assert_eq!(array_json, expected_array);

        let tuple_json = test_cases[1].to_json_value().expect("failed to serialize tuple");
        let expected_tuple = json!([42, true, "hello"]);
        assert_eq!(tuple_json, expected_tuple);

        let fixed_array_json =
            test_cases[2].to_json_value().expect("failed to serialize fixed array");
        // Current implementation treats FixedSizeArray same as Array
        let expected_fixed_array = json!([
            "0x0000000000000000000000000000000000000000000000000000000000000001",
            "0x0000000000000000000000000000000000000000000000000000000000000002",
            "0x0000000000000000000000000000000000000000000000000000000000000003"
        ]);
        assert_eq!(fixed_array_json, expected_fixed_array);

        let nested_json =
            test_cases[3].to_json_value().expect("failed to serialize nested structure");
        let expected_nested = json!([[10, 20], [false, 300]]);
        assert_eq!(nested_json, expected_nested);

        let struct_json = test_cases[5].to_json_value().expect("failed to serialize struct");
        let expected_struct = json!({
            "field1": 42,
            "field2": true,
            "nested_array": [1, 2]
        });
        assert_eq!(struct_json, expected_struct);

        let enum_json = test_cases[6].to_json_value().expect("failed to serialize enum");
        let expected_enum = json!({
            "VariantB": 123
        });
        assert_eq!(enum_json, expected_enum);

        let byte_array_json =
            test_cases[8].to_json_value().expect("failed to serialize byte array");
        assert_eq!(byte_array_json, json!("Hello, World!"));

        // Test empty collections
        let empty_array_json =
            test_cases[9].to_json_value().expect("failed to serialize empty array");
        assert_eq!(empty_array_json, json!([]));

        let empty_tuple_json =
            test_cases[10].to_json_value().expect("failed to serialize empty tuple");
        assert_eq!(empty_tuple_json, json!([]));

        let empty_fixed_array_json =
            test_cases[11].to_json_value().expect("failed to serialize empty fixed array");
        assert_eq!(empty_fixed_array_json, json!([]));

        // Round trip test for all cases
        for original in test_cases {
            // Convert to JSON value
            let json_value = original.to_json_value().expect("failed to serialize to JSON");

            // Create a new Ty of the same type structure but with None/empty values
            let mut parsed = create_empty_ty_like(&original);

            // Parse back from JSON value
            parsed.from_json_value(json_value.clone()).unwrap_or_else(|_| {
                panic!(
                    "failed to deserialize from JSON for type: {:?}, json: {}",
                    original.name(),
                    json_value
                )
            });

            // Should match original
            assert_eq!(parsed, original, "JSON round trip failed for type: {:?}", original.name());
        }
    }

    // Helper function to create empty Ty structures matching the shape of the original
    fn create_empty_ty_like(ty: &Ty) -> Ty {
        match ty {
            Ty::Primitive(p) => match p {
                Primitive::I8(_) => Ty::Primitive(Primitive::I8(None)),
                Primitive::I16(_) => Ty::Primitive(Primitive::I16(None)),
                Primitive::I32(_) => Ty::Primitive(Primitive::I32(None)),
                Primitive::I64(_) => Ty::Primitive(Primitive::I64(None)),
                Primitive::I128(_) => Ty::Primitive(Primitive::I128(None)),
                Primitive::U8(_) => Ty::Primitive(Primitive::U8(None)),
                Primitive::U16(_) => Ty::Primitive(Primitive::U16(None)),
                Primitive::U32(_) => Ty::Primitive(Primitive::U32(None)),
                Primitive::U64(_) => Ty::Primitive(Primitive::U64(None)),
                Primitive::U128(_) => Ty::Primitive(Primitive::U128(None)),
                Primitive::U256(_) => Ty::Primitive(Primitive::U256(None)),
                Primitive::Bool(_) => Ty::Primitive(Primitive::Bool(None)),
                Primitive::Felt252(_) => Ty::Primitive(Primitive::Felt252(None)),
                Primitive::ClassHash(_) => Ty::Primitive(Primitive::ClassHash(None)),
                Primitive::ContractAddress(_) => Ty::Primitive(Primitive::ContractAddress(None)),
                Primitive::EthAddress(_) => Ty::Primitive(Primitive::EthAddress(None)),
            },
            Ty::Struct(s) => Ty::Struct(Struct {
                name: s.name.clone(),
                children: s
                    .children
                    .iter()
                    .map(|m| Member {
                        name: m.name.clone(),
                        ty: create_empty_ty_like(&m.ty),
                        key: m.key,
                    })
                    .collect(),
            }),
            Ty::Enum(e) => Ty::Enum(Enum {
                name: e.name.clone(),
                option: None,
                options: e
                    .options
                    .iter()
                    .map(|opt| EnumOption {
                        name: opt.name.clone(),
                        ty: create_empty_ty_like(&opt.ty),
                    })
                    .collect(),
            }),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(create_empty_ty_like).collect()),
            Ty::Array(items) => {
                if items.is_empty() {
                    Ty::Array(vec![])
                } else {
                    // For arrays, we need at least one element as template
                    Ty::Array(vec![create_empty_ty_like(&items[0])])
                }
            }
            Ty::FixedSizeArray((items, size)) => {
                if items.is_empty() {
                    Ty::FixedSizeArray((vec![], *size))
                } else {
                    // For fixed size arrays, we need the correct number of elements
                    let empty_item = create_empty_ty_like(&items[0]);
                    Ty::FixedSizeArray((vec![empty_item; *size as usize], *size))
                }
            }
            Ty::ByteArray(_) => Ty::ByteArray(String::new()),
        }
    }
}
