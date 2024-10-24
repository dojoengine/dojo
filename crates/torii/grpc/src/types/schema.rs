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
            option: r#enum.option.unwrap_or_default() as u32,
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

impl TryFrom<proto::types::Primitive> for Primitive {
    type Error = SchemaError;
    fn try_from(primitive: proto::types::Primitive) -> Result<Self, Self::Error> {
        let value = primitive
            .primitive_type
            .ok_or(SchemaError::MissingExpectedData("primitive_type".to_string()))?;

        let primitive = match &value {
            proto::types::primitive::PrimitiveType::Bool(bool) => Primitive::Bool(Some(*bool)),
            proto::types::primitive::PrimitiveType::I8(int) => Primitive::I8(Some(*int as i8)),
            proto::types::primitive::PrimitiveType::I16(int) => Primitive::I16(Some(*int as i16)),
            proto::types::primitive::PrimitiveType::I32(int) => Primitive::I32(Some(*int)),
            proto::types::primitive::PrimitiveType::I64(int) => Primitive::I64(Some(*int)),
            proto::types::primitive::PrimitiveType::I128(bytes) => Primitive::I128(Some(
                i128::from_be_bytes(bytes.as_slice().try_into().map_err(SchemaError::FromSlice)?),
            )),
            proto::types::primitive::PrimitiveType::U8(int) => Primitive::U8(Some(*int as u8)),
            proto::types::primitive::PrimitiveType::U16(int) => Primitive::U16(Some(*int as u16)),
            proto::types::primitive::PrimitiveType::U32(int) => Primitive::U32(Some(*int)),
            proto::types::primitive::PrimitiveType::U64(int) => Primitive::U64(Some(*int)),
            proto::types::primitive::PrimitiveType::U128(bytes) => Primitive::U128(Some(
                u128::from_be_bytes(bytes.as_slice().try_into().map_err(SchemaError::FromSlice)?),
            )),
            proto::types::primitive::PrimitiveType::Usize(int) => Primitive::USize(Some(*int)),
            proto::types::primitive::PrimitiveType::Felt252(felt) => {
                Primitive::Felt252(Some(Felt::from_bytes_be_slice(felt.as_slice())))
            }
            proto::types::primitive::PrimitiveType::ClassHash(felt) => {
                Primitive::ClassHash(Some(Felt::from_bytes_be_slice(felt.as_slice())))
            }
            proto::types::primitive::PrimitiveType::ContractAddress(felt) => {
                Primitive::ContractAddress(Some(Felt::from_bytes_be_slice(felt.as_slice())))
            }
            proto::types::primitive::PrimitiveType::U256(bytes) => Primitive::U256(Some(
                U256::from_be_bytes(bytes.as_slice().try_into().map_err(SchemaError::FromSlice)?),
            )),
        };

        Ok(primitive)
    }
}

impl From<Primitive> for proto::types::Primitive {
    fn from(primitive: Primitive) -> Self {
        let value = match primitive {
            Primitive::Bool(bool) => {
                proto::types::primitive::PrimitiveType::Bool(bool.unwrap_or_default())
            }
            Primitive::I8(i8) => {
                proto::types::primitive::PrimitiveType::I8(i8.unwrap_or_default() as i32)
            }
            Primitive::I16(i16) => {
                proto::types::primitive::PrimitiveType::I16(i16.unwrap_or_default() as i32)
            }
            Primitive::I32(i32) => {
                proto::types::primitive::PrimitiveType::I32(i32.unwrap_or_default())
            }
            Primitive::I64(i64) => {
                proto::types::primitive::PrimitiveType::I64(i64.unwrap_or_default())
            }
            Primitive::I128(i128) => proto::types::primitive::PrimitiveType::I128(
                i128.unwrap_or_default().to_be_bytes().to_vec(),
            ),
            Primitive::U8(u8) => {
                proto::types::primitive::PrimitiveType::U8(u8.unwrap_or_default() as u32)
            }
            Primitive::U16(u16) => {
                proto::types::primitive::PrimitiveType::U16(u16.unwrap_or_default() as u32)
            }
            Primitive::U32(u32) => {
                proto::types::primitive::PrimitiveType::U32(u32.unwrap_or_default())
            }
            Primitive::U64(u64) => {
                proto::types::primitive::PrimitiveType::U64(u64.unwrap_or_default())
            }
            Primitive::U128(u128) => proto::types::primitive::PrimitiveType::U128(
                u128.unwrap_or_default().to_be_bytes().to_vec(),
            ),
            Primitive::USize(usize) => {
                proto::types::primitive::PrimitiveType::Usize(usize.unwrap_or_default())
            }
            Primitive::Felt252(felt) => proto::types::primitive::PrimitiveType::Felt252(
                felt.unwrap_or_default().to_bytes_be().to_vec(),
            ),
            Primitive::ClassHash(felt) => proto::types::primitive::PrimitiveType::ClassHash(
                felt.unwrap_or_default().to_bytes_be().to_vec(),
            ),
            Primitive::ContractAddress(felt) => {
                proto::types::primitive::PrimitiveType::ContractAddress(
                    felt.unwrap_or_default().to_bytes_be().to_vec(),
                )
            }
            Primitive::U256(u256) => proto::types::primitive::PrimitiveType::U256(
                u256.unwrap_or_default().to_be_bytes().to_vec(),
            ),
        };

        proto::types::Primitive { primitive_type: Some(value) }
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
