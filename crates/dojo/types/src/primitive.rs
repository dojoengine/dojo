use std::any::type_name;

use crypto_bigint::{Encoding, U256};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use starknet::core::types::Felt;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

use super::primitive_conversion::try_from_felt;

#[derive(
    AsRefStr,
    Display,
    EnumIter,
    EnumString,
    Copy,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Hash,
    Eq,
)]
#[serde(tag = "scalar_type", content = "value")]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Primitive {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    U256(U256),
    USize(u32),
    Bool(bool),
    Felt252(Felt),
    #[strum(serialize = "ClassHash")]
    ClassHash(Felt),
    #[strum(serialize = "ContractAddress")]
    ContractAddress(Felt),
}

#[derive(Debug, thiserror::Error)]
pub enum PrimitiveError {
    #[error("Value must have at least one FieldElement")]
    MissingFieldElement,
    #[error("Not enough FieldElements for U256")]
    NotEnoughFieldElements,
    #[error("Unsupported CairoType for SQL formatting")]
    UnsupportedType,
    #[error("Invalid byte length: {0}. expected {1}")]
    InvalidByteLength(usize, usize),
    #[error("Set value type mismatch")]
    TypeMismatch,
    #[error("Felt value ({value:#x}) out of range for {r#type}")]
    ValueOutOfRange { value: Felt, r#type: &'static str },
    #[error(transparent)]
    CairoSerde(#[from] cainome::cairo_serde::Error),
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    FeltFromFeltError(#[from] crate::primitive_conversion::PrimitiveFromFeltError),
}

#[derive(AsRefStr, Debug, Display, EnumString, PartialEq)]
#[strum(serialize_all = "UPPERCASE")]
pub enum SqlType {
    Integer,
    Text,
}

/// Macro to generate setter methods for Primitive enum variants.
macro_rules! set_primitive {
    ($method_name:ident, $variant:ident, $type:ty) => {
        /// Sets the inner value of the `Primitive` enum if variant matches.
        pub fn $method_name(&mut self, value: $type) -> Result<(), PrimitiveError> {
            match self {
                Primitive::$variant(_) => {
                    *self = Primitive::$variant(value);
                    Ok(())
                }
                _ => Err(PrimitiveError::TypeMismatch),
            }
        }
    };
}

/// Macro to generate getter methods for Primitive enum variants.
macro_rules! as_primitive {
    ($method_name:ident, $variant:ident, $type:ty) => {
        /// If the `Primitive` is variant type, returns the associated vartiant value. Returns
        /// `None` otherwise.
        pub fn $method_name(&self) -> Option<$type> {
            match self {
                Primitive::$variant(value) => Some(*value),
                _ => None,
            }
        }
    };
}

impl Primitive {
    as_primitive!(as_i8, I8, i8);
    as_primitive!(as_i16, I16, i16);
    as_primitive!(as_i32, I32, i32);
    as_primitive!(as_i64, I64, i64);
    as_primitive!(as_i128, I128, i128);
    as_primitive!(as_u8, U8, u8);
    as_primitive!(as_u16, U16, u16);
    as_primitive!(as_u32, U32, u32);
    as_primitive!(as_u64, U64, u64);
    as_primitive!(as_u128, U128, u128);
    as_primitive!(as_u256, U256, U256);
    as_primitive!(as_usize, USize, u32);
    as_primitive!(as_bool, Bool, bool);
    as_primitive!(as_felt252, Felt252, Felt);
    as_primitive!(as_class_hash, ClassHash, Felt);
    as_primitive!(as_contract_address, ContractAddress, Felt);

    set_primitive!(set_i8, I8, i8);
    set_primitive!(set_i16, I16, i16);
    set_primitive!(set_i32, I32, i32);
    set_primitive!(set_i64, I64, i64);
    set_primitive!(set_i128, I128, i128);
    set_primitive!(set_u8, U8, u8);
    set_primitive!(set_u16, U16, u16);
    set_primitive!(set_u32, U32, u32);
    set_primitive!(set_u64, U64, u64);
    set_primitive!(set_u128, U128, u128);
    set_primitive!(set_u256, U256, U256);
    set_primitive!(set_usize, USize, u32);
    set_primitive!(set_bool, Bool, bool);
    set_primitive!(set_felt252, Felt252, Felt);
    set_primitive!(set_class_hash, ClassHash, Felt);
    set_primitive!(set_contract_address, ContractAddress, Felt);

    pub fn to_numeric(&self) -> usize {
        match self {
            Primitive::U8(_) => 0,
            Primitive::U16(_) => 1,
            Primitive::U32(_) => 2,
            Primitive::U64(_) => 3,
            Primitive::U128(_) => 4,
            Primitive::U256(_) => 5,
            Primitive::USize(_) => 6,
            Primitive::Bool(_) => 7,
            Primitive::Felt252(_) => 8,
            Primitive::ClassHash(_) => 9,
            Primitive::ContractAddress(_) => 10,
            Primitive::I8(_) => 11,
            Primitive::I16(_) => 12,
            Primitive::I32(_) => 13,
            Primitive::I64(_) => 14,
            Primitive::I128(_) => 15,
        }
    }

    pub fn from_numeric(value: usize) -> Option<Self> {
        Self::iter().nth(value)
    }

    pub fn to_sql_type(&self) -> SqlType {
        match self {
            // sqlite integer is 64-bit signed integer
            Primitive::I8(_)
            | Primitive::I16(_)
            | Primitive::I32(_)
            | Primitive::I64(_)
            | Primitive::U8(_)
            | Primitive::U16(_)
            | Primitive::U32(_)
            | Primitive::USize(_)
            | Primitive::Bool(_) => SqlType::Integer,

            // u64 cannot fit into a i64, so we use text
            Primitive::U64(_)
            | Primitive::I128(_)
            | Primitive::U128(_)
            | Primitive::U256(_)
            | Primitive::ContractAddress(_)
            | Primitive::ClassHash(_)
            | Primitive::Felt252(_) => SqlType::Text,
        }
    }

    pub fn to_sql_value(&self) -> String {
        match self {
            // Integers
            Primitive::I8(i8) => format!("{}", i8),
            Primitive::I16(i16) => format!("{}", i16),
            Primitive::I32(i32) => format!("{}", i32),
            Primitive::I64(i64) => format!("{}", i64),

            Primitive::U8(u8) => format!("{}", u8),
            Primitive::U16(u16) => format!("{}", u16),
            Primitive::U32(u32) => format!("{}", u32),
            Primitive::USize(u32) => format!("{}", u32),
            Primitive::Bool(bool) => format!("{}", bool),

            // Hex string
            Primitive::I128(i128) => format!("{:#064x}", i128),
            Primitive::ContractAddress(felt) => format!("{:#064x}", felt),
            Primitive::ClassHash(felt) => format!("{:#064x}", felt),
            Primitive::Felt252(felt) => format!("{:#064x}", felt),
            Primitive::U128(u128) => format!("{:#064x}", u128),
            Primitive::U64(u64) => format!("{:#064x}", u64),

            Primitive::U256(u256) => format!("0x{:064x}", u256),
        }
    }

    pub fn deserialize(&mut self, felts: &mut Vec<Felt>) -> Result<(), PrimitiveError> {
        if felts.is_empty() {
            return Err(PrimitiveError::MissingFieldElement);
        }

        match self {
            Primitive::I8(ref mut value) => {
                let felt = felts.remove(0);
                *value = try_from_felt::<i8>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i8>(), value: felt }
                })?;
            }

            Primitive::I16(ref mut value) => {
                let felt = felts.remove(0);
                *value = try_from_felt::<i16>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i16>(), value: felt }
                })?;
            }

            Primitive::I32(ref mut value) => {
                let felt = felts.remove(0);
                *value = try_from_felt::<i32>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i32>(), value: felt }
                })?;
            }

            Primitive::I64(ref mut value) => {
                let felt = felts.remove(0);
                *value = try_from_felt::<i64>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i64>(), value: felt }
                })?;
            }

            Primitive::I128(ref mut value) => {
                let felt = felts.remove(0);
                *value = try_from_felt::<i128>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i128>(), value: felt }
                })?;
            }

            Primitive::U8(ref mut value) => {
                let felt = felts.remove(0);
                *value = felt.to_u8().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u8>(),
                    value: felt,
                })?;
            }

            Primitive::U16(ref mut value) => {
                let felt = felts.remove(0);
                *value = felt.to_u16().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u16>(),
                    value: felt,
                })?;
            }

            Primitive::U32(ref mut value) => {
                let felt = felts.remove(0);
                *value = felt.to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u32>(),
                    value: felt,
                })?;
            }

            Primitive::U64(ref mut value) => {
                let felt = felts.remove(0);
                *value = felt.to_u64().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u64>(),
                    value: felt,
                })?;
            }

            Primitive::U128(ref mut value) => {
                let felt = felts.remove(0);
                *value = felt.to_u128().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u128>(),
                    value: felt,
                })?;
            }

            Primitive::U256(ref mut value) => {
                if felts.len() < 2 {
                    return Err(PrimitiveError::NotEnoughFieldElements);
                }
                let value0 = felts.remove(0);
                let value1 = felts.remove(0);
                let value0_bytes = value0.to_bytes_be();
                let value1_bytes = value1.to_bytes_be();
                let mut bytes = [0u8; 32];
                bytes[16..].copy_from_slice(&value0_bytes[16..]);
                bytes[..16].copy_from_slice(&value1_bytes[16..]);
                *value = U256::from_be_bytes(bytes);
            }

            Primitive::USize(ref mut value) => {
                let felt = felts.remove(0);
                *value = felt.to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u32>(),
                    value: felt,
                })?;
            }

            Primitive::Bool(ref mut value) => {
                let raw = felts.remove(0);
                *value = raw == Felt::ONE;
            }

            Primitive::ContractAddress(ref mut value) => {
                *value = felts.remove(0);
            }

            Primitive::ClassHash(ref mut value) => {
                *value = felts.remove(0);
            }

            Primitive::Felt252(ref mut value) => {
                *value = felts.remove(0);
            }
        }

        Ok(())
    }

    pub fn serialize(&self) -> Vec<Felt> {
        match self {
            Primitive::I8(value) => vec![Felt::from(*value)],
            Primitive::I16(value) => vec![Felt::from(*value)],
            Primitive::I32(value) => vec![Felt::from(*value)],
            Primitive::I64(value) => vec![Felt::from(*value)],
            Primitive::I128(value) => vec![Felt::from(*value)],
            Primitive::U8(value) => vec![Felt::from(*value)],
            Primitive::U16(value) => vec![Felt::from(*value)],
            Primitive::U32(value) => vec![Felt::from(*value)],
            Primitive::U64(value) => vec![Felt::from(*value)],
            Primitive::U128(value) => vec![Felt::from(*value)],
            Primitive::U256(value) => {
                let bytes: [u8; 32] = value.to_be_bytes();
                let value0_slice = &bytes[16..];
                let value1_slice = &bytes[..16];
                let mut value0_array = [0u8; 32];
                let mut value1_array = [0u8; 32];
                value0_array[16..].copy_from_slice(value0_slice);
                value1_array[16..].copy_from_slice(value1_slice);
                let value0 = Felt::from_bytes_be(&value0_array);
                let value1 = Felt::from_bytes_be(&value1_array);
                vec![value0, value1]
            }
            Primitive::USize(value) => vec![Felt::from(*value)],
            Primitive::Bool(value) => vec![if *value { Felt::ONE } else { Felt::ZERO }],
            Primitive::ContractAddress(value) => vec![Felt::from(*value)],
            Primitive::ClassHash(value) => vec![Felt::from(*value)],
            Primitive::Felt252(value) => vec![Felt::from(*value)]
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crypto_bigint::U256;
    use starknet::core::types::Felt;

    use super::Primitive;

    #[test]
    fn test_u256() {
        let primitive = Primitive::U256(U256::from_be_hex(
            "aaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbccccccccccccccccdddddddddddddddd",
        ));
        let sql_value = primitive.to_sql_value();
        let serialized = primitive.serialize();

        let mut deserialized = primitive;
        deserialized.deserialize(&mut serialized.clone()).unwrap();

        assert_eq!(sql_value, "0xaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbccccccccccccccccdddddddddddddddd");
        assert_eq!(
            serialized,
            vec![
                Felt::from_str("0xccccccccccccccccdddddddddddddddd").unwrap(),
                Felt::from_str("0xaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbb").unwrap()
            ]
        );
        assert_eq!(deserialized, primitive)
    }

    #[test]
    fn inner_value_getter_setter() {
        let mut primitive = Primitive::I8(0);
        primitive.set_i8(-1).unwrap();
        assert_eq!(primitive.as_i8(), Some(-1i8));
        let mut primitive = Primitive::I16(0);
        primitive.set_i16(-1i16).unwrap();
        assert_eq!(primitive.as_i16(), Some(-1i16));
        let mut primitive = Primitive::I32(0);
        primitive.set_i32(-1i32).unwrap();
        assert_eq!(primitive.as_i32(), Some(-1i32));
        let mut primitive = Primitive::I64(0);
        primitive.set_i64(-1i64).unwrap();
        assert_eq!(primitive.as_i64(), Some(-1i64));
        let mut primitive = Primitive::I128(0);
        primitive.set_i128(-1i128).unwrap();
        assert_eq!(primitive.as_i128(), Some(-1i128));
        let mut primitive = Primitive::U8(0);
        primitive.set_u8(1u8).unwrap();
        assert_eq!(primitive.as_u8(), Some(1u8));
        let mut primitive = Primitive::U16(0);
        primitive.set_u16(1u16).unwrap();
        assert_eq!(primitive.as_u16(), Some(1u16));
        let mut primitive = Primitive::U32(0);
        primitive.set_u32(1u32).unwrap();
        assert_eq!(primitive.as_u32(), Some(1u32));
        let mut primitive = Primitive::U64(0);
        primitive.set_u64(1u64).unwrap();
        assert_eq!(primitive.as_u64(), Some(1u64));
        let mut primitive = Primitive::U128(0);
        primitive.set_u128(1u128).unwrap();
        assert_eq!(primitive.as_u128(), Some(1u128));
        let mut primitive = Primitive::U256(U256::ZERO);
        primitive.set_u256(U256::from(1u128)).unwrap();
        assert_eq!(primitive.as_u256(), Some(U256::from(1u128)));
        let mut primitive = Primitive::USize(0);
        primitive.set_usize(1u32).unwrap();
        assert_eq!(primitive.as_usize(), Some(1u32));
        let mut primitive = Primitive::Bool(false);
        primitive.set_bool(true).unwrap();
        assert_eq!(primitive.as_bool(), Some(true));
        let mut primitive = Primitive::Felt252(Felt::ZERO);
        primitive.set_felt252(Felt::from(1u128)).unwrap();
        assert_eq!(primitive.as_felt252(), Some(Felt::from(1u128)));
        let mut primitive = Primitive::ClassHash(Felt::ZERO);
        primitive.set_class_hash(Felt::from(1u128)).unwrap();
        assert_eq!(primitive.as_class_hash(), Some(Felt::from(1u128)));
        let mut primitive = Primitive::ContractAddress(Felt::ZERO);
        primitive.set_contract_address(Felt::from(1u128)).unwrap();
        assert_eq!(primitive.as_contract_address(), Some(Felt::from(1u128)));
    }

    #[test]
    fn test_primitive_deserialization() {
        let test_cases = vec![
            (vec![Felt::from(-42i8)], Primitive::I8(-42)),
            (vec![Felt::from(-1000i16)], Primitive::I16(-1000)),
            (vec![Felt::from(-100000i32)], Primitive::I32(-100000)),
            (vec![Felt::from(-1000000000i64)], Primitive::I64(-1000000000)),
            (
                vec![Felt::from(-1000000000000000000i128)],
                Primitive::I128(-1000000000000000000),
            ),
            (vec![Felt::from(42u8)], Primitive::U8(42)),
            (vec![Felt::from(1000u16)], Primitive::U16(1000)),
            (vec![Felt::from(100000u32)], Primitive::U32(100000)),
            (vec![Felt::from(1000000000u64)], Primitive::U64(1000000000)),
            (vec![Felt::from(1000000000000000000u128)], Primitive::U128(1000000000000000000)),
            (vec![Felt::from(42u32)], Primitive::USize(42)),
            (vec![Felt::from(1u8)], Primitive::Bool(true)),
            (vec![Felt::from(123456789u128)], Primitive::Felt252(Felt::from(123456789))),
            (vec![Felt::from(987654321u128)], Primitive::ClassHash(Felt::from(987654321))),
            (
                vec![Felt::from(123456789u128)],
                Primitive::ContractAddress(Felt::from(123456789)),
            ),
        ];

        for (serialized, expected) in test_cases {
            let mut to_deser = expected;
            to_deser.deserialize(&mut serialized.clone()).unwrap();
            assert_eq!(to_deser, expected);
        }
    }
}
