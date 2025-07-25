use std::any::type_name;
use std::str::FromStr;

use cainome::cairo_serde::{ByteArray, CairoSerde};
use crypto_bigint::U256;
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
    FixedSizeArray(Vec<(Ty, u32)>),
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
            Ty::FixedSizeArray(ty) => {
                if let Some((ty, size)) = ty.first() {
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
    pub fn as_fixed_size_array(&self) -> Option<&Vec<(Ty, u32)>> {
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
                Ty::FixedSizeArray(items_ty) => {
                    let (item_ty, size) = &items_ty[0];
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

    pub fn deserialize(&mut self, felts: &mut Vec<Felt>) -> Result<(), PrimitiveError> {
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
            Ty::FixedSizeArray(items_ty) => {
                let (item_ty, size) = &items_ty[0];
                for _ in 0..*size {
                    let mut cur_item_ty = item_ty.clone();
                    cur_item_ty.deserialize(felts)?;
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
            Ty::Primitive(primitive) => match primitive {
                Primitive::Bool(Some(v)) => Ok(json!(*v)),
                Primitive::I8(Some(v)) => Ok(json!(*v)),
                Primitive::I16(Some(v)) => Ok(json!(*v)),
                Primitive::I32(Some(v)) => Ok(json!(*v)),
                Primitive::I64(Some(_)) => Ok(json!(primitive.to_sql_value())),
                Primitive::I128(Some(_)) => Ok(json!(primitive.to_sql_value())),
                Primitive::U8(Some(v)) => Ok(json!(*v)),
                Primitive::U16(Some(v)) => Ok(json!(*v)),
                Primitive::U32(Some(v)) => Ok(json!(*v)),
                Primitive::U64(Some(_)) => Ok(json!(primitive.to_sql_value())),
                Primitive::U128(Some(_)) => Ok(json!(primitive.to_sql_value())),
                Primitive::U256(Some(_)) => Ok(json!(primitive.to_sql_value())),
                Primitive::Felt252(Some(_)) => Ok(json!(primitive.to_sql_value())),
                Primitive::ClassHash(Some(_)) => Ok(json!(primitive.to_sql_value())),
                Primitive::ContractAddress(Some(_)) => Ok(json!(primitive.to_sql_value())),
                Primitive::EthAddress(Some(_)) => Ok(json!(primitive.to_sql_value())),
                _ => Err(PrimitiveError::MissingFieldElement),
            },
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
            Ty::Array(items) => {
                let values: Result<Vec<_>, _> = items.iter().map(|ty| ty.to_json_value()).collect();
                Ok(json!(values?))
            }
            Ty::FixedSizeArray(items) => {
                let values: Result<Vec<_>, _> =
                    items.iter().map(|(ty, _)| ty.to_json_value()).collect();
                Ok(json!(values?))
            }
            Ty::Tuple(items) => {
                let values: Result<Vec<_>, _> = items.iter().map(|ty| ty.to_json_value()).collect();
                Ok(json!(values?))
            }
            Ty::ByteArray(bytes) => Ok(json!(bytes.clone())),
        }
    }

    /// Parse a JSON Value into a Ty
    pub fn from_json_value(&mut self, value: JsonValue) -> Result<(), PrimitiveError> {
        match (self, value) {
            (Ty::Primitive(primitive), value) => match primitive {
                Primitive::Bool(v) => {
                    if let JsonValue::Bool(b) = value {
                        *v = Some(b);
                    }
                }
                Primitive::I8(v) => {
                    if let JsonValue::Number(n) = value {
                        *v = n.as_i64().map(|n| n as i8);
                    }
                }
                Primitive::I16(v) => {
                    if let JsonValue::Number(n) = value {
                        *v = n.as_i64().map(|n| n as i16);
                    }
                }
                Primitive::I32(v) => {
                    if let JsonValue::Number(n) = value {
                        *v = n.as_i64().map(|n| n as i32);
                    }
                }
                Primitive::I64(v) => {
                    if let JsonValue::String(s) = value {
                        *v = s.parse().ok();
                    }
                }
                Primitive::I128(v) => {
                    if let JsonValue::String(s) = value {
                        *v = s.parse().ok();
                    }
                }
                Primitive::U8(v) => {
                    if let JsonValue::Number(n) = value {
                        *v = n.as_u64().map(|n| n as u8);
                    }
                }
                Primitive::U16(v) => {
                    if let JsonValue::Number(n) = value {
                        *v = n.as_u64().map(|n| n as u16);
                    }
                }
                Primitive::U32(v) => {
                    if let JsonValue::Number(n) = value {
                        *v = n.as_u64().map(|n| n as u32);
                    }
                }
                Primitive::U64(v) => {
                    if let JsonValue::String(s) = value {
                        *v = s.parse().ok();
                    }
                }
                Primitive::U128(v) => {
                    if let JsonValue::String(s) = value {
                        *v = s.parse().ok();
                    }
                }
                Primitive::U256(v) => {
                    if let JsonValue::String(s) = value {
                        *v = Some(U256::from_be_hex(s.trim_start_matches("0x")));
                    }
                }
                Primitive::Felt252(v) => {
                    if let JsonValue::String(s) = value {
                        *v = Felt::from_str(&s).ok();
                    }
                }
                Primitive::ClassHash(v) => {
                    if let JsonValue::String(s) = value {
                        *v = Felt::from_str(&s).ok();
                    }
                }
                Primitive::ContractAddress(v) => {
                    if let JsonValue::String(s) = value {
                        *v = Felt::from_str(&s).ok();
                    }
                }
                Primitive::EthAddress(v) => {
                    if let JsonValue::String(s) = value {
                        *v = Felt::from_str(&s).ok();
                    }
                }
            },
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
                let template = items[0].clone();
                items.clear();
                for value in values {
                    let mut item = template.clone();
                    item.from_json_value(value)?;
                    items.push(item);
                }
            }
            (Ty::FixedSizeArray(items), JsonValue::Array(values)) => {
                let (template, length) = items[0].clone();
                items.clear();
                for value in values {
                    let mut item = template.clone();
                    item.from_json_value(value)?;
                    items.push((item, length));
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
                Ty::FixedSizeArray(items_ty) => {
                    let (item_ty, length) = &items_ty[0];
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
}
