use crypto_bigint::{Encoding, U256};
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;

use crate::proto::{self};

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("Missing expected data")]
    MissingExpectedData,
    #[error("Unsupported type")]
    UnsupportedType,
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
            ty: member.ty.ok_or(SchemaError::MissingExpectedData)?.try_into()?,
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
            ty: option.ty.ok_or(SchemaError::MissingExpectedData)?.try_into()?,
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
            .ok_or(SchemaError::MissingExpectedData)?
            .value_type
            .ok_or(SchemaError::MissingExpectedData)?;

        let primitive = match &value_type {
            proto::types::value::ValueType::BoolValue(bool) => Primitive::Bool(Some(*bool)),
            proto::types::value::ValueType::UintValue(int) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::U8) => Primitive::U8(Some(*int as u8)),
                    Some(proto::types::PrimitiveType::U16) => Primitive::U16(Some(*int as u16)),
                    Some(proto::types::PrimitiveType::U32) => Primitive::U32(Some(*int as u32)),
                    Some(proto::types::PrimitiveType::U64) => Primitive::U64(Some(*int)),
                    Some(proto::types::PrimitiveType::Usize) => Primitive::USize(Some(*int as u32)),
                    _ => return Err(SchemaError::UnsupportedType),
                }
            }
            proto::types::value::ValueType::ByteValue(bytes) => {
                match proto::types::PrimitiveType::from_i32(primitive_type) {
                    Some(proto::types::PrimitiveType::U128)
                    | Some(proto::types::PrimitiveType::Felt252)
                    | Some(proto::types::PrimitiveType::ClassHash)
                    | Some(proto::types::PrimitiveType::ContractAddress) => {
                        Primitive::Felt252(Some(Felt::from_bytes_be_slice(bytes)))
                    }
                    Some(proto::types::PrimitiveType::U256) => {
                        Primitive::U256(Some(U256::from_be_slice(bytes)))
                    }
                    _ => return Err(SchemaError::UnsupportedType),
                }
            }
            _ => {
                return Err(SchemaError::UnsupportedType);
            }
        };

        Ok(primitive)
    }
}

impl From<Primitive> for proto::types::Primitive {
    fn from(primitive: Primitive) -> Self {
        use proto::types::value::ValueType;

        let value_type = match primitive {
            Primitive::Bool(bool) => bool.map(ValueType::BoolValue),
            Primitive::U8(u8) => u8.map(|val| ValueType::UintValue(val as u64)),
            Primitive::U16(u16) => u16.map(|val| ValueType::UintValue(val as u64)),
            Primitive::U32(u32) => u32.map(|val| ValueType::UintValue(val as u64)),
            Primitive::U64(u64) => u64.map(ValueType::UintValue),
            Primitive::USize(usize) => usize.map(|val| ValueType::UintValue(val as u64)),
            Primitive::U128(u128) => {
                u128.map(|val| ValueType::ByteValue(val.to_be_bytes().to_vec()))
            }
            Primitive::U256(u256) => {
                u256.map(|val| ValueType::ByteValue(val.to_be_bytes().to_vec()))
            }
            Primitive::Felt252(felt) => {
                felt.map(|val| ValueType::ByteValue(val.to_bytes_be().to_vec()))
            }
            Primitive::ClassHash(class) => {
                class.map(|val| ValueType::ByteValue(val.to_bytes_be().to_vec()))
            }
            Primitive::ContractAddress(contract) => {
                contract.map(|val| ValueType::ByteValue(val.to_bytes_be().to_vec()))
            }
        }
        .expect("value expected");

        proto::types::Primitive {
            value: Some(proto::types::Value { value_type: Some(value_type) }),
            r#type: primitive.to_numeric() as i32,
        }
    }
}

impl TryFrom<proto::types::Ty> for Ty {
    type Error = SchemaError;
    fn try_from(ty: proto::types::Ty) -> Result<Self, Self::Error> {
        match ty.ty_type.ok_or(SchemaError::MissingExpectedData)? {
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
