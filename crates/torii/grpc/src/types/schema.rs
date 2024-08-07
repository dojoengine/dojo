use std::str::FromStr;

use crypto_bigint::{Encoding, U256};
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use serde::{Deserialize, Serialize};
use starknet::core::types::{Felt, FromStrError};

use crate::proto::{self};

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("Missing expected data: {0}")]
    MissingExpectedData(String),
    #[error("Unsupported primitive type for {0}")]
    UnsupportedType(String),
    #[error("Invalid byte length: {0}. Expected: {1}")]
    InvalidByteLength(usize, usize),
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    FromSlice(#[from] std::array::TryFromSliceError),
    #[error(transparent)]
    FromStr(#[from] FromStrError),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Hash, Eq, Clone)]
pub struct Entity {
    pub hashed_keys: Felt,
    pub models: Vec<Struct>,
}

impl TryFrom<proto::types::Entity> for Entity {
    type Error = SchemaError;
    fn try_from(entity: proto::types::Entity) -> Result<Self, Self::Error> {
        Ok(Self {
            hashed_keys: Felt::from_bytes_be_slice(&entity.hashed_keys),
            models: entity
                .models
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl From<Ty> for proto::types::Ty {
    fn from(ty: Ty) -> Self {
        let ty_type = match ty {
            Ty::Primitive(primitive) => Some(proto::types::ty::TyType::Primitive(primitive.into())),
            Ty::Enum(r#enum) => Some(proto::types::ty::TyType::Enum(r#enum.into())),
            Ty::Struct(r#struct) => Some(proto::types::ty::TyType::Struct(r#struct.into())),
            Ty::Tuple(tuple) => Some(proto::types::ty::TyType::Tuple(proto::types::Array {
                children: tuple.into_iter().map(Into::into).collect::<Vec<_>>(),
            })),
            Ty::Array(array) => Some(proto::types::ty::TyType::Array(proto::types::Array {
                children: array.into_iter().map(Into::into).collect::<Vec<_>>(),
            })),
            Ty::ByteArray(string) => Some(proto::types::ty::TyType::Bytearray(string)),
        };

        proto::types::Ty { ty_type }
    }
}

impl TryFrom<proto::types::Member> for Member {
    type Error = SchemaError;
    fn try_from(member: proto::types::Member) -> Result<Self, Self::Error> {
        Ok(Member {
            name: member.name,
            ty: member.ty.ok_or(SchemaError::MissingExpectedData("ty".to_string()))?.try_into()?,
            key: member.key,
        })
    }
}

impl From<Member> for proto::types::Member {
    fn from(member: Member) -> Self {
        proto::types::Member { name: member.name, ty: Some(member.ty.into()), key: member.key }
    }
}

impl TryFrom<proto::types::EnumOption> for EnumOption {
    type Error = SchemaError;
    fn try_from(option: proto::types::EnumOption) -> Result<Self, Self::Error> {
        Ok(EnumOption {
            name: option.name,
            ty: option.ty.ok_or(SchemaError::MissingExpectedData("ty".to_string()))?.try_into()?,
        })
    }
}

impl From<EnumOption> for proto::types::EnumOption {
    fn from(option: EnumOption) -> Self {
        proto::types::EnumOption { name: option.name, ty: Some(option.ty.into()) }
    }
}

impl TryFrom<proto::types::Enum> for Enum {
    type Error = SchemaError;
    fn try_from(r#enum: proto::types::Enum) -> Result<Self, Self::Error> {
        Ok(Enum {
            name: r#enum.name.clone(),
            option: Some(r#enum.option as u8),
            options: r#enum
                .options
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl From<Enum> for proto::types::Enum {
    fn from(r#enum: Enum) -> Self {
        proto::types::Enum {
            name: r#enum.name,
            option: r#enum.option.expect("option value") as u32,
            options: r#enum.options.into_iter().map(Into::into).collect::<Vec<_>>(),
        }
    }
}

impl TryFrom<proto::types::Struct> for Struct {
    type Error = SchemaError;
    fn try_from(r#struct: proto::types::Struct) -> Result<Self, Self::Error> {
        Ok(Struct {
            name: r#struct.name,
            children: r#struct
                .children
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl From<Struct> for proto::types::Struct {
    fn from(r#struct: Struct) -> Self {
        proto::types::Struct {
            name: r#struct.name,
            children: r#struct.children.into_iter().map(Into::into).collect::<Vec<_>>(),
        }
    }
}

// FIX: weird catch-22 issue - prost Enum has `try_from` trait we can use, however, using it results
// in wasm compile err about From<i32> missing. Implementing that trait results in clippy error
// about duplicate From<i32>... Workaround is to use deprecated `from_i32` and allow deprecation
// warning.
#[allow(deprecated)]
impl TryFrom<proto::types::Primitive> for Primitive {
    type Error = SchemaError;
    fn try_from(primitive: proto::types::Primitive) -> Result<Self, Self::Error> {
        let primitive_type = primitive.r#type;
        let value_type = primitive
            .value
            .ok_or(SchemaError::MissingExpectedData("value".to_string()))?
            .value_type
            .ok_or(SchemaError::MissingExpectedData("value_type".to_string()))?;

        let primitive = match &value_type {
            proto::types::value::ValueType::BoolValue(bool) => Primitive::Bool(Some(*bool)),
            proto::types::value::ValueType::UintValue(int) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::I8) => Primitive::I8(Some(*int as i8)),
                    Some(proto::types::PrimitiveType::I16) => Primitive::I16(Some(*int as i16)),
                    Some(proto::types::PrimitiveType::I32) => Primitive::I32(Some(*int as i32)),
                    Some(proto::types::PrimitiveType::I64) => Primitive::I64(Some(*int as i64)),
                    Some(proto::types::PrimitiveType::I128) => Primitive::I128(Some(*int as i128)),
                    Some(proto::types::PrimitiveType::U8) => Primitive::U8(Some(*int as u8)),
                    Some(proto::types::PrimitiveType::U16) => Primitive::U16(Some(*int as u16)),
                    Some(proto::types::PrimitiveType::U32) => Primitive::U32(Some(*int as u32)),
                    Some(proto::types::PrimitiveType::U64) => Primitive::U64(Some(*int)),
                    Some(proto::types::PrimitiveType::U128) => Primitive::U128(Some(*int as u128)),
                    Some(proto::types::PrimitiveType::Usize) => Primitive::USize(Some(*int as u32)),
                    _ => return Err(SchemaError::UnsupportedType("UintValue".to_string())),
                }
            }
            proto::types::value::ValueType::IntValue(int) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::I8) => Primitive::I8(Some(*int as i8)),
                    Some(proto::types::PrimitiveType::I16) => Primitive::I16(Some(*int as i16)),
                    Some(proto::types::PrimitiveType::I32) => Primitive::I32(Some(*int as i32)),
                    Some(proto::types::PrimitiveType::I64) => Primitive::I64(Some(*int)),
                    Some(proto::types::PrimitiveType::I128) => Primitive::I128(Some(*int as i128)),
                    Some(proto::types::PrimitiveType::U8) => Primitive::U8(Some(*int as u8)),
                    Some(proto::types::PrimitiveType::U16) => Primitive::U16(Some(*int as u16)),
                    Some(proto::types::PrimitiveType::U32) => Primitive::U32(Some(*int as u32)),
                    Some(proto::types::PrimitiveType::U64) => Primitive::U64(Some(*int as u64)),
                    Some(proto::types::PrimitiveType::U128) => Primitive::U128(Some(*int as u128)),
                    Some(proto::types::PrimitiveType::Usize) => Primitive::USize(Some(*int as u32)),
                    _ => return Err(SchemaError::UnsupportedType("IntValue".to_string())),
                }
            }
            proto::types::value::ValueType::ByteValue(bytes) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::I128) => {
                        Primitive::I128(Some(i128::from_be_bytes(
                            bytes.as_slice().try_into().map_err(SchemaError::FromSlice)?,
                        )))
                    }
                    Some(proto::types::PrimitiveType::U128) => {
                        Primitive::U128(Some(u128::from_be_bytes(
                            bytes.as_slice().try_into().map_err(SchemaError::FromSlice)?,
                        )))
                    }
                    Some(proto::types::PrimitiveType::U256) => {
                        Primitive::U256(Some(U256::from_be_slice(bytes.as_slice())))
                    }
                    Some(proto::types::PrimitiveType::Felt252) => {
                        Primitive::Felt252(Some(Felt::from_bytes_be_slice(bytes.as_slice())))
                    }
                    Some(proto::types::PrimitiveType::ClassHash) => {
                        Primitive::ClassHash(Some(Felt::from_bytes_be_slice(bytes.as_slice())))
                    }
                    Some(proto::types::PrimitiveType::ContractAddress) => {
                        Primitive::ContractAddress(Some(Felt::from_bytes_be_slice(
                            bytes.as_slice(),
                        )))
                    }
                    _ => return Err(SchemaError::UnsupportedType("ByteValue".to_string())),
                }
            }
            proto::types::value::ValueType::StringValue(str) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::I8) => {
                        Primitive::I8(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::I16) => {
                        Primitive::I16(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::I32) => {
                        Primitive::I32(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::I64) => {
                        Primitive::I64(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::I128) => {
                        Primitive::I128(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::U8) => {
                        Primitive::U8(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::U16) => {
                        Primitive::U16(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::U32) => {
                        Primitive::U32(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::U64) => {
                        Primitive::U64(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::U128) => {
                        Primitive::U128(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::Usize) => {
                        Primitive::USize(Some(str.parse().map_err(SchemaError::ParseIntError)?))
                    }
                    Some(proto::types::PrimitiveType::Felt252) => {
                        Primitive::Felt252(Some(Felt::from_str(str).map_err(SchemaError::FromStr)?))
                    }
                    Some(proto::types::PrimitiveType::ClassHash) => Primitive::ClassHash(Some(
                        Felt::from_str(str).map_err(SchemaError::FromStr)?,
                    )),
                    Some(proto::types::PrimitiveType::ContractAddress) => {
                        Primitive::ContractAddress(Some(
                            Felt::from_str(str).map_err(SchemaError::FromStr)?,
                        ))
                    }
                    _ => return Err(SchemaError::UnsupportedType("StringValue".to_string())),
                }
            }
        };

        Ok(primitive)
    }
}

impl From<Primitive> for proto::types::Primitive {
    fn from(primitive: Primitive) -> Self {
        use proto::types::value::ValueType;

        let value_type = match primitive {
            Primitive::I8(i8) => ValueType::IntValue(i8.unwrap_or_default() as i64),
            Primitive::I16(i16) => ValueType::IntValue(i16.unwrap_or_default() as i64),
            Primitive::I32(i32) => ValueType::IntValue(i32.unwrap_or_default() as i64),
            Primitive::I64(i64) => ValueType::IntValue(i64.unwrap_or_default()),
            Primitive::I128(i128) => {
                ValueType::ByteValue(i128.unwrap_or_default().to_be_bytes().to_vec())
            }
            Primitive::U8(u8) => ValueType::UintValue(u8.unwrap_or_default() as u64),
            Primitive::U16(u16) => ValueType::UintValue(u16.unwrap_or_default() as u64),
            Primitive::U32(u32) => ValueType::UintValue(u32.unwrap_or_default() as u64),
            Primitive::U64(u64) => ValueType::UintValue(u64.unwrap_or_default()),
            Primitive::U128(u128) => {
                ValueType::ByteValue(u128.unwrap_or_default().to_be_bytes().to_vec())
            }
            Primitive::U256(u256) => {
                ValueType::ByteValue(u256.unwrap_or_default().to_be_bytes().to_vec())
            }
            Primitive::USize(usize) => ValueType::UintValue(usize.unwrap_or_default() as u64),
            Primitive::Bool(bool) => ValueType::BoolValue(bool.unwrap_or_default()),
            Primitive::Felt252(felt)
            | Primitive::ClassHash(felt)
            | Primitive::ContractAddress(felt) => {
                ValueType::ByteValue(felt.unwrap_or_default().to_bytes_be().to_vec())
            }
        };

        proto::types::Primitive {
            value: Some(proto::types::Value { value_type: Some(value_type) }),
            r#type: primitive.to_numeric() as i32,
        }
    }
}

impl TryFrom<proto::types::Ty> for Ty {
    type Error = SchemaError;
    fn try_from(ty: proto::types::Ty) -> Result<Self, Self::Error> {
        match ty.ty_type.ok_or(SchemaError::MissingExpectedData("ty_type".to_string()))? {
            proto::types::ty::TyType::Primitive(primitive) => {
                Ok(Ty::Primitive(primitive.try_into()?))
            }
            proto::types::ty::TyType::Struct(r#struct) => Ok(Ty::Struct(r#struct.try_into()?)),
            proto::types::ty::TyType::Enum(r#enum) => Ok(Ty::Enum(r#enum.try_into()?)),
            proto::types::ty::TyType::Tuple(array) => Ok(Ty::Tuple(
                array.children.into_iter().map(TryInto::try_into).collect::<Result<Vec<_>, _>>()?,
            )),
            proto::types::ty::TyType::Array(array) => Ok(Ty::Array(
                array.children.into_iter().map(TryInto::try_into).collect::<Result<Vec<_>, _>>()?,
            )),
            proto::types::ty::TyType::Bytearray(string) => Ok(Ty::ByteArray(string)),
        }
    }
}
