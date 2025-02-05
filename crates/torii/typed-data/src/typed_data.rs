use std::str::FromStr;

use cainome::cairo_serde::ByteArray;
use crypto_bigint::{Encoding, U256};
use dojo_types::primitive::Primitive;
use dojo_types::schema::Ty;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Number;
use starknet::core::types::Felt;
use starknet::core::utils::{cairo_short_string_to_felt, get_selector_from_name};
use starknet_crypto::poseidon_hash_many;

use crate::error::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimpleField {
    pub name: String,
    pub r#type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParentField {
    pub name: String,
    pub r#type: String,
    pub contains: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Field {
    ParentType(ParentField),
    SimpleType(SimpleField),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PrimitiveType {
    // All of object types. Including preset types
    Object(IndexMap<String, PrimitiveType>),
    Array(Vec<PrimitiveType>),
    Bool(bool),
    // comprehensive representation of
    // String, ShortString, Selector and Felt
    String(String),
    // For JSON numbers. Formed into a Felt
    Number(Number),
}

fn get_preset_types() -> IndexMap<String, Vec<Field>> {
    let mut types = IndexMap::new();

    types.insert(
        "TokenAmount".to_string(),
        vec![
            Field::SimpleType(SimpleField {
                name: "token_address".to_string(),
                r#type: "ContractAddress".to_string(),
            }),
            Field::SimpleType(SimpleField {
                name: "amount".to_string(),
                r#type: "u256".to_string(),
            }),
        ],
    );

    types.insert(
        "NftId".to_string(),
        vec![
            Field::SimpleType(SimpleField {
                name: "collection_address".to_string(),
                r#type: "ContractAddress".to_string(),
            }),
            Field::SimpleType(SimpleField {
                name: "token_id".to_string(),
                r#type: "u256".to_string(),
            }),
        ],
    );

    types.insert(
        "u256".to_string(),
        vec![
            Field::SimpleType(SimpleField { name: "low".to_string(), r#type: "u128".to_string() }),
            Field::SimpleType(SimpleField { name: "high".to_string(), r#type: "u128".to_string() }),
        ],
    );

    types
}

// Get the fields of a specific type
// Looks up both the types hashmap as well as the preset types
// Returns the fields and the hashmap of types
fn get_fields(name: &str, types: &IndexMap<String, Vec<Field>>) -> Result<Vec<Field>, Error> {
    types.get(name).cloned().ok_or_else(|| Error::TypeNotFound(name.to_string()))
}

fn get_dependencies(
    name: &str,
    types: &IndexMap<String, Vec<Field>>,
    dependencies: &mut Vec<String>,
) -> Result<(), Error> {
    if dependencies.contains(&name.to_string()) {
        return Ok(());
    }

    dependencies.push(name.to_string());

    for field in get_fields(name, types)? {
        let mut field_type = match field {
            Field::SimpleType(simple_field) => simple_field.r#type.clone(),
            Field::ParentType(parent_field) => parent_field.contains.clone(),
        };

        field_type = field_type.trim_end_matches('*').to_string();

        if types.contains_key(&field_type) && !dependencies.contains(&field_type) {
            get_dependencies(&field_type, types, dependencies)?;
        }
    }

    Ok(())
}

pub fn encode_type(name: &str, types: &IndexMap<String, Vec<Field>>) -> Result<String, Error> {
    let mut type_hash = String::new();

    // get dependencies
    let mut dependencies: Vec<String> = Vec::new();
    get_dependencies(name, types, &mut dependencies)?;

    // sort dependencies
    dependencies.sort_by_key(|dep| dep.to_lowercase());

    for dep in dependencies {
        type_hash += &format!("\"{}\"", dep);

        type_hash += "(";

        let fields = get_fields(&dep, types)?;
        for (idx, field) in fields.iter().enumerate() {
            match field {
                Field::SimpleType(simple_field) => {
                    // if ( at start and ) at end
                    if simple_field.r#type.starts_with('(') && simple_field.r#type.ends_with(')') {
                        let inner_types =
                            &simple_field.r#type[1..simple_field.r#type.len() - 1]
                                .split(',')
                                .map(|t| {
                                    if !t.is_empty() { format!("\"{}\"", t) } else { t.to_string() }
                                })
                                .collect::<Vec<String>>()
                                .join(",");
                        type_hash += &format!("\"{}\":({})", simple_field.name, inner_types);
                    } else {
                        type_hash +=
                            &format!("\"{}\":\"{}\"", simple_field.name, simple_field.r#type);
                    }
                }
                Field::ParentType(parent_field) => {
                    type_hash +=
                        &format!("\"{}\":\"{}\"", parent_field.name, parent_field.contains);
                }
            }

            if idx < fields.len() - 1 {
                type_hash += ",";
            }
        }

        type_hash += ")";
    }

    Ok(type_hash)
}

#[derive(Debug, Default)]
pub struct Ctx {
    pub base_type: String,
    pub parent_type: String,
    pub is_preset: bool,
}

pub(crate) struct FieldInfo {
    _name: String,
    r#type: String,
    base_type: String,
    index: usize,
}

pub(crate) fn get_value_type(
    name: &str,
    types: &IndexMap<String, Vec<Field>>,
) -> Result<FieldInfo, Error> {
    // iter both "types" and "preset_types" to find the field
    for (idx, (key, value)) in types.iter().enumerate() {
        if key == name {
            return Ok(FieldInfo {
                _name: name.to_string(),
                r#type: key.clone(),
                base_type: "".to_string(),
                index: idx,
            });
        }

        for (idx, field) in value.iter().enumerate() {
            match field {
                Field::SimpleType(simple_field) => {
                    if simple_field.name == name {
                        return Ok(FieldInfo {
                            _name: name.to_string(),
                            r#type: simple_field.r#type.clone(),
                            base_type: "".to_string(),
                            index: idx,
                        });
                    }
                }
                Field::ParentType(parent_field) => {
                    if parent_field.name == name {
                        return Ok(FieldInfo {
                            _name: name.to_string(),
                            r#type: parent_field.contains.clone(),
                            base_type: parent_field.r#type.clone(),
                            index: idx,
                        });
                    }
                }
            }
        }
    }

    Err(Error::FieldNotFound(name.to_string()))
}

fn get_hex(value: &str) -> Result<Felt, Error> {
    Felt::from_str(value).or_else(|_| {
        cairo_short_string_to_felt(value)
            .map_err(|e| Error::ParseError(format!("Invalid shortstring for felt: {}", e)))
    })
}

impl PrimitiveType {
    pub fn encode(
        &self,
        r#type: &str,
        types: &IndexMap<String, Vec<Field>>,
        preset_types: &IndexMap<String, Vec<Field>>,
        ctx: &mut Ctx,
    ) -> Result<Felt, Error> {
        match self {
            PrimitiveType::Object(obj) => {
                ctx.is_preset = preset_types.contains_key(r#type);

                let mut hashes = Vec::new();

                if ctx.base_type == "enum" {
                    let (variant_name, value) = obj.first().ok_or_else(|| {
                        Error::InvalidEnum("Enum value must be populated".to_string())
                    })?;

                    let variant_type = get_value_type(variant_name, types)?;

                    // variant index
                    hashes.push(Felt::from(variant_type.index as u32));

                    match value {
                        PrimitiveType::Array(arr) =>
                        // variant parameters
                        {
                            for (idx, param) in arr.iter().enumerate() {
                                let field_type = &variant_type
                                    .r#type
                                    .trim_start_matches('(')
                                    .trim_end_matches(')')
                                    .split(',')
                                    .nth(idx)
                                    .ok_or_else(|| {
                                        Error::InvalidEnum("Invalid enum variant type".to_string())
                                    })?;

                                let field_hash =
                                    param.encode(field_type, types, preset_types, ctx)?;
                                hashes.push(field_hash);
                            }
                        }
                        _ => hashes.push(value.encode(
                            variant_type.r#type.as_str(),
                            types,
                            preset_types,
                            ctx,
                        )?),
                    };

                    return Ok(poseidon_hash_many(hashes.as_slice()));
                }

                let type_hash =
                    encode_type(r#type, if ctx.is_preset { preset_types } else { types })?;
                hashes.push(get_selector_from_name(&type_hash).map_err(|e| {
                    Error::ParseError(format!("Invalid type {} for selector: {}", r#type, e))
                })?);

                for (field_name, value) in obj {
                    // recheck if we're currently in a preset type
                    ctx.is_preset = preset_types.contains_key(r#type);

                    // pass correct types - preset or types
                    let field_type = get_value_type(
                        field_name,
                        if ctx.is_preset { preset_types } else { types },
                    )?;
                    ctx.base_type = field_type.base_type;
                    ctx.parent_type = r#type.to_string();
                    let field_hash =
                        value.encode(field_type.r#type.as_str(), types, preset_types, ctx)?;
                    hashes.push(field_hash);
                }

                Ok(poseidon_hash_many(hashes.as_slice()))
            }
            PrimitiveType::Array(array) => match r#type {
                // tuple
                _ if r#type.starts_with('(') && r#type.ends_with(')') => {
                    let inner_types = r#type[1..r#type.len() - 1]
                        .split(',')
                        .map(|t| t.trim())
                        .collect::<Vec<&str>>();

                    if inner_types.len() != array.len() {
                        return Err(Error::InvalidValue("Tuple length mismatch".to_string()));
                    }

                    let mut hashes = Vec::new();
                    for (idx, value) in array.iter().enumerate() {
                        let field_hash =
                            value.encode(inner_types[idx], types, preset_types, ctx)?;
                        hashes.push(field_hash);
                    }

                    Ok(poseidon_hash_many(hashes.as_slice()))
                }
                // array
                _ => Ok(poseidon_hash_many(
                    array
                        .iter()
                        .map(|x| x.encode(r#type.trim_end_matches('*'), types, preset_types, ctx))
                        .collect::<Result<Vec<_>, _>>()?
                        .as_slice(),
                )),
            },
            PrimitiveType::Bool(boolean) => {
                let v = if *boolean { Felt::from(1_u32) } else { Felt::from(0_u32) };
                Ok(v)
            }
            PrimitiveType::String(string) => match r#type {
                "shortstring" => get_hex(string),
                "string" => {
                    // split the string into short strings and encode
                    let byte_array = ByteArray::from_string(string).map_err(|e| {
                        Error::ParseError(format!("Invalid string for bytearray: {}", e))
                    })?;

                    let mut hashes = vec![Felt::from(byte_array.data.len())];

                    for hash in byte_array.data {
                        hashes.push(hash.felt());
                    }

                    hashes.push(byte_array.pending_word);
                    hashes.push(Felt::from(byte_array.pending_word_len));

                    Ok(poseidon_hash_many(hashes.as_slice()))
                }
                "selector" => get_selector_from_name(string)
                    .map_err(|e| Error::ParseError(format!("Invalid selector: {}", e))),
                "felt" => get_hex(string),
                "ContractAddress" => get_hex(string),
                "ClassHash" => get_hex(string),
                "timestamp" => get_hex(string),
                "u128" => get_hex(string),
                "i128" => get_hex(string),
                _ => Err(Error::InvalidType(format!("Invalid type {} for string", r#type))),
            },
            PrimitiveType::Number(number) => {
                let felt = Felt::from_str(&number.to_string())
                    .map_err(|_| Error::ParseError(format!("Invalid number {}", number)))?;
                Ok(felt)
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    pub version: String,
    #[serde(rename = "chainId")]
    pub chain_id: String,
    pub revision: Option<String>,
}

impl Domain {
    pub fn new(name: &str, version: &str, chain_id: &str, revision: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            chain_id: chain_id.to_string(),
            revision: revision.map(|s| s.to_string()),
        }
    }

    pub fn encode(&self, types: &IndexMap<String, Vec<Field>>) -> Result<Felt, Error> {
        if self.revision.as_deref().unwrap_or("1") != "1" {
            return Err(Error::InvalidDomain("Legacy revision 0 is not supported".to_string()));
        }

        let mut object = IndexMap::new();
        object.insert("name".to_string(), PrimitiveType::String(self.name.clone()));
        object.insert("version".to_string(), PrimitiveType::String(self.version.clone()));
        object.insert("chainId".to_string(), PrimitiveType::String(self.chain_id.clone()));

        if let Some(revision) = &self.revision {
            object.insert("revision".to_string(), PrimitiveType::String(revision.clone()));
        }

        PrimitiveType::Object(object).encode(
            "StarknetDomain",
            types,
            &IndexMap::new(),
            &mut Default::default(),
        )
    }
}

macro_rules! from_str {
    ($string:expr, $type:ty) => {
        if $string.starts_with("0x") || $string.starts_with("0X") {
            <$type>::from_str_radix(&$string[2..], 16)
        } else {
            <$type>::from_str($string)
        }
        .map_err(|e| Error::ParseError(format!("Failed to parse number: {}", e)))
    };
}

pub fn parse_value_to_ty(value: &PrimitiveType, ty: &mut Ty) -> Result<(), Error> {
    match value {
        PrimitiveType::Object(object) => match ty {
            Ty::Struct(struct_) => {
                for (key, value) in object {
                    let member =
                        struct_.children.iter_mut().find(|member| member.name == *key).ok_or_else(
                            || Error::FieldNotFound(format!("Member {} not found", key)),
                        )?;

                    parse_value_to_ty(value, &mut member.ty)?;
                }
            }
            // U256 is an object with two u128 fields
            // low and high
            Ty::Primitive(Primitive::U256(u256)) => {
                let mut low = Ty::Primitive(Primitive::U128(None));
                let mut high = Ty::Primitive(Primitive::U128(None));

                // parse the low and high fields
                parse_value_to_ty(&object["low"], &mut low)?;
                parse_value_to_ty(&object["high"], &mut high)?;

                let low = low.as_primitive().unwrap().as_u128().unwrap();
                let high = high.as_primitive().unwrap().as_u128().unwrap();

                let mut bytes = [0u8; 32];
                bytes[..16].copy_from_slice(&high.to_be_bytes());
                bytes[16..].copy_from_slice(&low.to_be_bytes());

                *u256 = Some(U256::from_be_slice(&bytes));
            }
            // an enum is a SNIP-12 compliant object with a single key
            // where the K is the variant name
            // and the value is the variant value
            Ty::Enum(enum_) => {
                let (option_name, value) = object
                    .first()
                    .ok_or_else(|| Error::InvalidEnum("Enum variant not found".to_string()))?;

                enum_.options.iter_mut().for_each(|option| {
                    if option.name == *option_name {
                        parse_value_to_ty(value, &mut option.ty).unwrap();
                    }
                });

                enum_
                    .set_option(option_name)
                    .map_err(|e| Error::InvalidEnum(format!("Failed to set enum option: {}", e)))?;
            }
            _ => {
                return Err(Error::InvalidType(format!("Invalid object type for {}", ty.name())));
            }
        },
        PrimitiveType::Array(values) => match ty {
            Ty::Array(array) => {
                let inner_type = array[0].clone();

                // clear the array, which contains the inner type
                array.clear();

                // parse each value to the inner type
                for value in values {
                    let mut ty = inner_type.clone();
                    parse_value_to_ty(value, &mut ty)?;
                    array.push(ty);
                }
            }
            Ty::Tuple(tuple) => {
                // our array values need to match the length of the tuple
                if tuple.len() != values.len() {
                    return Err(Error::InvalidValue("Tuple length mismatch".to_string()));
                }

                for (i, value) in tuple.iter_mut().enumerate() {
                    parse_value_to_ty(&values[i], value)?;
                }
            }
            _ => {
                return Err(Error::InvalidType(format!("Invalid array type for {}", ty.name())));
            }
        },
        PrimitiveType::Number(number) => match ty {
            Ty::Primitive(primitive) => match *primitive {
                Primitive::I8(ref mut i8) => {
                    *i8 = Some(number.as_i64().unwrap() as i8);
                }
                Primitive::I16(ref mut i16) => {
                    *i16 = Some(number.as_i64().unwrap() as i16);
                }
                Primitive::I32(ref mut i32) => {
                    *i32 = Some(number.as_i64().unwrap() as i32);
                }
                Primitive::I64(ref mut i64) => {
                    *i64 = Some(number.as_i64().unwrap());
                }
                Primitive::U8(ref mut u8) => {
                    *u8 = Some(number.as_u64().unwrap() as u8);
                }
                Primitive::U16(ref mut u16) => {
                    *u16 = Some(number.as_u64().unwrap() as u16);
                }
                Primitive::U32(ref mut u32) => {
                    *u32 = Some(number.as_u64().unwrap() as u32);
                }
                Primitive::U64(ref mut u64) => {
                    *u64 = Some(number.as_u64().unwrap());
                }
                _ => {
                    return Err(Error::InvalidType(format!(
                        "Invalid number type for {}",
                        ty.name()
                    )));
                }
            },
            _ => {
                return Err(Error::InvalidType(format!("Invalid number type for {}", ty.name())));
            }
        },
        PrimitiveType::Bool(boolean) => {
            *ty = Ty::Primitive(Primitive::Bool(Some(*boolean)));
        }
        PrimitiveType::String(string) => match ty {
            Ty::Primitive(primitive) => match primitive {
                Primitive::I8(v) => {
                    *v = Some(from_str!(string, i8)?);
                }
                Primitive::I16(v) => {
                    *v = Some(from_str!(string, i16)?);
                }
                Primitive::I32(v) => {
                    *v = Some(from_str!(string, i32)?);
                }
                Primitive::I64(v) => {
                    *v = Some(from_str!(string, i64)?);
                }
                Primitive::I128(v) => {
                    *v = Some(from_str!(string, i128)?);
                }
                Primitive::U8(v) => {
                    *v = Some(from_str!(string, u8)?);
                }
                Primitive::U16(v) => {
                    *v = Some(from_str!(string, u16)?);
                }
                Primitive::U32(v) => {
                    *v = Some(from_str!(string, u32)?);
                }
                Primitive::U64(v) => {
                    *v = Some(from_str!(string, u64)?);
                }
                Primitive::U128(v) => {
                    *v = Some(from_str!(string, u128)?);
                }
                Primitive::Felt252(v) => {
                    *v = Some(Felt::from_str(string).unwrap());
                }
                Primitive::ClassHash(v) => {
                    *v = Some(Felt::from_str(string).unwrap());
                }
                Primitive::ContractAddress(v) => {
                    *v = Some(Felt::from_str(string).unwrap());
                }
                Primitive::EthAddress(v) => {
                    *v = Some(Felt::from_str(string).unwrap());
                }
                Primitive::Bool(v) => {
                    *v = Some(bool::from_str(string).unwrap());
                }
                _ => {
                    return Err(Error::InvalidType("Invalid primitive type".to_string()));
                }
            },
            Ty::ByteArray(s) => {
                s.clone_from(string);
            }
            _ => {
                return Err(Error::InvalidType(format!("Invalid string type for {}", ty.name())));
            }
        },
    }

    Ok(())
}

pub fn map_ty_to_primitive(ty: &Ty) -> Result<PrimitiveType, Error> {
    match ty {
        Ty::Struct(struct_) => {
            let mut object = IndexMap::new();
            for member in &struct_.children {
                object.insert(member.name.clone(), map_ty_to_primitive(&member.ty)?);
            }
            Ok(PrimitiveType::Object(object))
        }
        Ty::Enum(enum_) => {
            let mut object = IndexMap::new();
            let option =
                enum_.option.ok_or(Error::InvalidEnum("Enum option not found".to_string()))?;
            let option = enum_
                .options
                .get(option as usize)
                .ok_or(Error::InvalidEnum("Enum option not found".to_string()))?;
            object.insert(option.name.clone(), map_ty_to_primitive(&option.ty)?);
            Ok(PrimitiveType::Object(object))
        }
        Ty::Array(array) => {
            let values: Result<Vec<PrimitiveType>, Error> =
                array.iter().map(map_ty_to_primitive).collect();
            Ok(PrimitiveType::Array(values?))
        }
        Ty::Tuple(tuple) => {
            let values: Result<Vec<PrimitiveType>, Error> =
                tuple.iter().map(map_ty_to_primitive).collect();
            Ok(PrimitiveType::Array(values?))
        }
        Ty::Primitive(primitive) => match primitive {
            Primitive::Bool(b) => Ok(PrimitiveType::Bool(b.unwrap_or(false))),
            Primitive::I8(n) => Ok(PrimitiveType::Number(Number::from(n.unwrap_or(0)))),
            Primitive::I16(n) => Ok(PrimitiveType::Number(Number::from(n.unwrap_or(0)))),
            Primitive::I32(n) => Ok(PrimitiveType::Number(Number::from(n.unwrap_or(0)))),
            Primitive::I64(n) => Ok(PrimitiveType::String((n.unwrap_or(0)).to_string())),
            Primitive::I128(n) => Ok(PrimitiveType::String((n.unwrap_or(0)).to_string())),
            Primitive::U8(n) => Ok(PrimitiveType::Number(Number::from(n.unwrap_or(0)))),
            Primitive::U16(n) => Ok(PrimitiveType::Number(Number::from(n.unwrap_or(0)))),
            Primitive::U32(n) => Ok(PrimitiveType::Number(Number::from(n.unwrap_or(0)))),
            Primitive::U64(n) => Ok(PrimitiveType::String((n.unwrap_or(0)).to_string())),
            Primitive::U128(n) => Ok(PrimitiveType::String((n.unwrap_or(0)).to_string())),
            Primitive::Felt252(f) => {
                Ok(PrimitiveType::String((f.unwrap_or(Felt::ZERO)).to_string()))
            }
            Primitive::ClassHash(c) => {
                Ok(PrimitiveType::String((c.unwrap_or(Felt::ZERO)).to_string()))
            }
            Primitive::ContractAddress(c) => {
                Ok(PrimitiveType::String((c.unwrap_or(Felt::ZERO)).to_string()))
            }
            Primitive::EthAddress(e) => {
                Ok(PrimitiveType::String((e.unwrap_or(Felt::ZERO)).to_string()))
            }
            Primitive::U256(u256) => {
                let mut object = IndexMap::new();
                let bytes = u256.map_or([0u8; 32], |u256| u256.to_be_bytes());
                let high = u128::from_be_bytes(bytes[..16].try_into().unwrap());
                let low = u128::from_be_bytes(bytes[16..].try_into().unwrap());
                object.insert("high".to_string(), PrimitiveType::String(high.to_string()));
                object.insert("low".to_string(), PrimitiveType::String(low.to_string()));
                Ok(PrimitiveType::Object(object))
            }
        },
        Ty::ByteArray(s) => Ok(PrimitiveType::String(s.clone())),
    }
}

fn map_ty_type(types: &mut IndexMap<String, Vec<Field>>, name: &str, ty: Ty) -> Field {
    match ty {
        Ty::Primitive(primitive) => match primitive {
            // map all signed integers to i128
            Primitive::I8(_)
            | Primitive::I16(_)
            | Primitive::I32(_)
            | Primitive::I64(_)
            | Primitive::I128(_) => Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: "i128".to_string(),
            }),
            Primitive::U8(_)
            | Primitive::U16(_)
            | Primitive::U32(_)
            | Primitive::U64(_)
            | Primitive::U128(_) => Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: "u128".to_string(),
            }),
            Primitive::U256(_) => Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: "u256".to_string(),
            }),
            Primitive::Bool(_) => Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: "bool".to_string(),
            }),
            Primitive::Felt252(_) => Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: "felt".to_string(),
            }),
            Primitive::ClassHash(_) => Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: "ClassHash".to_string(),
            }),
            Primitive::ContractAddress(_) => Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: "ContractAddress".to_string(),
            }),
            Primitive::EthAddress(_) => Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: "EthAddress".to_string(),
            }),
        },
        Ty::Array(array) => {
            // if array is empty, we fallback to felt
            let array_type = if let Some(inner) = array.first() {
                map_ty_type(types, "inner", inner.clone())
            } else {
                return Field::SimpleType(SimpleField {
                    name: name.to_string(),
                    r#type: "felt".to_string(),
                });
            };

            Field::SimpleType(SimpleField {
                name: name.to_string(),
                r#type: format!(
                    "{}*",
                    match array_type {
                        Field::SimpleType(simple_field) => simple_field.r#type,
                        Field::ParentType(parent_field) => parent_field.r#type,
                    }
                ),
            })
        }
        Ty::Struct(struct_ty) => {
            let mut fields = Vec::new();
            for member in struct_ty.children.iter() {
                let field = map_ty_type(types, &member.name, member.ty.clone());
                fields.push(field);
            }

            types.insert(struct_ty.name.clone(), fields);

            Field::SimpleType(SimpleField { name: name.to_string(), r#type: struct_ty.name })
        }
        Ty::Enum(enum_ty) => {
            let mut fields = Vec::new();
            for option in enum_ty.options.iter() {
                let field = map_ty_type(types, &option.name, option.ty.clone());
                fields.push(field);
            }

            types.insert(enum_ty.name.clone(), fields);

            Field::ParentType(ParentField {
                name: name.to_string(),
                r#type: "enum".to_string(),
                contains: enum_ty.name,
            })
        }
        Ty::Tuple(tuple_ty) => Field::SimpleType(SimpleField {
            name: name.to_string(),
            r#type: format!(
                "({})",
                tuple_ty
                    .iter()
                    .map(|ty| match map_ty_type(types, "inner", ty.clone()) {
                        Field::SimpleType(simple_field) => simple_field.r#type,
                        Field::ParentType(parent_field) => parent_field.r#type,
                    })
                    .collect::<Vec<String>>()
                    .join(",")
            ),
        }),
        Ty::ByteArray(_) => {
            Field::SimpleType(SimpleField { name: name.to_string(), r#type: "string".to_string() })
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypedData {
    pub types: IndexMap<String, Vec<Field>>,
    #[serde(rename = "primaryType")]
    pub primary_type: String,
    pub domain: Domain,
    pub message: IndexMap<String, PrimitiveType>,
}

impl TypedData {
    pub fn new(
        types: IndexMap<String, Vec<Field>>,
        primary_type: &str,
        domain: Domain,
        message: IndexMap<String, PrimitiveType>,
    ) -> Self {
        Self { types, primary_type: primary_type.to_string(), domain, message }
    }

    pub fn from_model(model: Ty, domain: Domain) -> Result<Self, Error> {
        let model = model.as_struct().expect("Model must be a struct");
        let mut types = IndexMap::new();
        types.insert(
            "StarknetDomain".to_string(),
            vec![
                Field::SimpleType(SimpleField {
                    name: "name".to_string(),
                    r#type: "shortstring".to_string(),
                }),
                Field::SimpleType(SimpleField {
                    name: "version".to_string(),
                    r#type: "shortstring".to_string(),
                }),
                Field::SimpleType(SimpleField {
                    name: "chainId".to_string(),
                    r#type: "shortstring".to_string(),
                }),
                Field::SimpleType(SimpleField {
                    name: "revision".to_string(),
                    r#type: "shortstring".to_string(),
                }),
            ],
        );

        let mut values = IndexMap::new();

        let mut fields = Vec::new();
        for member in model.children.iter() {
            let field = map_ty_type(&mut types, &member.name, member.ty.clone());
            fields.push(field);

            values.insert(member.name.clone(), map_ty_to_primitive(&member.ty)?);
        }

        types.insert(model.name.clone(), fields);

        Ok(Self::new(types, model.name.as_str(), domain, values))
    }

    pub fn encode(&self, account: Felt) -> Result<Felt, Error> {
        let preset_types = get_preset_types();

        let prefix_message = cairo_short_string_to_felt("StarkNet Message")
            .map_err(|e| Error::CryptoError(e.to_string()))?;

        let domain_hash = self.domain.encode(&self.types)?;

        let message_hash = PrimitiveType::Object(self.message.clone()).encode(
            &self.primary_type,
            &self.types,
            &preset_types,
            &mut Default::default(),
        )?;

        Ok(poseidon_hash_many(&[prefix_message, domain_hash, account, message_hash]))
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::utils::starknet_keccak;
    use starknet_crypto::Felt;

    use super::*;

    #[test]
    fn test_read_json() {
        // deserialize from json file
        let path = "mocks/mail_StructArray.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        println!("{:?}", typed_data);

        let path = "mocks/example_enum.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        println!("{:?}", typed_data);

        let path = "mocks/example_presetTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        println!("{:?}", typed_data);
    }

    #[test]
    fn test_type_encode() {
        let path = "mocks/example_baseTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let encoded = encode_type(&typed_data.primary_type, &typed_data.types).unwrap();

        assert_eq!(
            encoded,
            "\"Example\"(\"n0\":\"felt\",\"n1\":\"bool\",\"n2\":\"string\",\"n3\":\"selector\",\"\
             n4\":\"u128\",\"n5\":\"ContractAddress\",\"n6\":\"ClassHash\",\"n7\":\"timestamp\",\"\
             n8\":\"shortstring\")"
        );

        let path = "mocks/mail_StructArray.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let encoded = encode_type(&typed_data.primary_type, &typed_data.types).unwrap();

        assert_eq!(
            encoded,
            "\"Mail\"(\"from\":\"Person\",\"to\":\"Person\",\"posts_len\":\"felt\",\"posts\":\"\
             Post*\")\"Person\"(\"name\":\"felt\",\"wallet\":\"felt\")\"Post\"(\"title\":\"felt\",\
             \"content\":\"felt\")"
        );

        let path = "mocks/example_enum.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let encoded = encode_type(&typed_data.primary_type, &typed_data.types).unwrap();

        assert_eq!(
            encoded,
            "\"Example\"(\"someEnum\":\"MyEnum\")\"MyEnum\"(\"Variant 1\":(),\"Variant \
             2\":(\"u128\",\"u128*\"),\"Variant 3\":(\"u128\"))"
        );

        let path = "mocks/example_presetTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let encoded = encode_type(&typed_data.primary_type, &typed_data.types).unwrap();

        assert_eq!(encoded, "\"Example\"(\"n0\":\"TokenAmount\",\"n1\":\"NftId\")");
    }

    #[test]
    fn test_selector_encode() {
        let selector = PrimitiveType::String("transfer".to_string());
        let selector_hash =
            PrimitiveType::String(starknet_keccak("transfer".as_bytes()).to_string());

        let types = IndexMap::new();
        let preset_types = get_preset_types();

        let encoded_selector =
            selector.encode("selector", &types, &preset_types, &mut Default::default()).unwrap();
        let raw_encoded_selector =
            selector_hash.encode("felt", &types, &preset_types, &mut Default::default()).unwrap();

        assert_eq!(encoded_selector, raw_encoded_selector);
        assert_eq!(encoded_selector, starknet_keccak("transfer".as_bytes()));
    }

    #[test]
    fn test_domain_hash() {
        let path = "mocks/example_baseTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let domain_hash = typed_data.domain.encode(&typed_data.types).unwrap();

        assert_eq!(
            domain_hash,
            Felt::from_hex("0x555f72e550b308e50c1a4f8611483a174026c982a9893a05c185eeb85399657")
                .unwrap()
        );
    }

    #[test]
    fn test_message_hash() {
        let address = Felt::from_hex("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826").unwrap();

        let path = "mocks/example_baseTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let message_hash = typed_data.encode(address).unwrap();

        assert_eq!(
            message_hash,
            Felt::from_hex("0x790d9fa99cf9ad91c515aaff9465fcb1c87784d9cfb27271ed193675cd06f9c")
                .unwrap()
        );

        let path = "mocks/example_enum.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let message_hash = typed_data.encode(address).unwrap();

        assert_eq!(
            message_hash,
            Felt::from_hex("0x3df10475ad5a8f49db4345a04a5b09164d2e24b09f6e1e236bc1ccd87627cc")
                .unwrap()
        );

        let path = "mocks/example_presetTypes.json";
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);

        let typed_data: TypedData = serde_json::from_reader(reader).unwrap();

        let message_hash = typed_data.encode(address).unwrap();

        assert_eq!(
            message_hash,
            Felt::from_hex("0x26e7b8cedfa63cdbed14e7e51b60ee53ac82bdf26724eb1e3f0710cb8987522")
                .unwrap()
        );
    }
}
