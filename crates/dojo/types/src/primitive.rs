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
    PartialOrd,
    Ord,
)]
#[serde(tag = "scalar_type", content = "value")]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Primitive {
    I8(Option<i8>),
    I16(Option<i16>),
    I32(Option<i32>),
    I64(Option<i64>),
    I128(Option<i128>),
    U8(Option<u8>),
    U16(Option<u16>),
    U32(Option<u32>),
    U64(Option<u64>),
    U128(Option<u128>),
    U256(Option<U256>),
    Bool(Option<bool>),
    Felt252(Option<Felt>),
    #[strum(serialize = "ClassHash")]
    ClassHash(Option<Felt>),
    #[strum(serialize = "ContractAddress")]
    ContractAddress(Option<Felt>),
    #[strum(serialize = "EthAddress")]
    EthAddress(Option<Felt>),
}

#[derive(Debug, thiserror::Error)]
pub enum PrimitiveError {
    #[error("Invalid enum selector `{actual_selector}`")]
    InvalidEnumSelector {
        /// The actual selector value that was invalid.
        actual_selector: u8,
    },

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
    #[error("Invalid SQL value format: {0}")]
    InvalidSqlValue(String),
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
        pub fn $method_name(&mut self, value: Option<$type>) -> Result<(), PrimitiveError> {
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
                Primitive::$variant(value) => *value,
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
    as_primitive!(as_bool, Bool, bool);
    as_primitive!(as_felt252, Felt252, Felt);
    as_primitive!(as_class_hash, ClassHash, Felt);
    as_primitive!(as_contract_address, ContractAddress, Felt);
    as_primitive!(as_eth_address, EthAddress, Felt);

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
    set_primitive!(set_bool, Bool, bool);
    set_primitive!(set_felt252, Felt252, Felt);
    set_primitive!(set_class_hash, ClassHash, Felt);
    set_primitive!(set_contract_address, ContractAddress, Felt);
    set_primitive!(set_eth_address, EthAddress, Felt);

    pub fn to_numeric(&self) -> usize {
        match self {
            Primitive::Bool(_) => 0,
            Primitive::U8(_) => 1,
            Primitive::U16(_) => 2,
            Primitive::U32(_) => 3,
            Primitive::U64(_) => 4,
            Primitive::U128(_) => 5,
            Primitive::U256(_) => 6,
            Primitive::I8(_) => 7,
            Primitive::I16(_) => 8,
            Primitive::I32(_) => 9,
            Primitive::I64(_) => 10,
            Primitive::I128(_) => 11,
            Primitive::Felt252(_) => 12,
            Primitive::ClassHash(_) => 13,
            Primitive::ContractAddress(_) => 14,
            Primitive::EthAddress(_) => 15,
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
            | Primitive::Bool(_) => SqlType::Integer,

            // u64 cannot fit into a i64, so we use text
            Primitive::U64(_)
            | Primitive::I128(_)
            | Primitive::U128(_)
            | Primitive::U256(_)
            | Primitive::ContractAddress(_)
            | Primitive::ClassHash(_)
            | Primitive::Felt252(_)
            | Primitive::EthAddress(_) => SqlType::Text,
        }
    }

    pub fn to_sql_value(&self) -> String {
        match self {
            // Integers
            Primitive::I8(i8) => format!("{}", i8.unwrap_or_default()),
            Primitive::I16(i16) => format!("{}", i16.unwrap_or_default()),
            Primitive::I32(i32) => format!("{}", i32.unwrap_or_default()),
            Primitive::I64(i64) => format!("{}", i64.unwrap_or_default()),

            Primitive::U8(u8) => format!("{}", u8.unwrap_or_default()),
            Primitive::U16(u16) => format!("{}", u16.unwrap_or_default()),
            Primitive::U32(u32) => format!("{}", u32.unwrap_or_default()),
            Primitive::Bool(bool) => format!("{}", bool.unwrap_or_default() as i32),

            // Hex string
            Primitive::I128(i128) => format!("0x{:064x}", i128.unwrap_or_default()),
            Primitive::ContractAddress(felt) => format!("0x{:064x}", felt.unwrap_or_default()),
            Primitive::ClassHash(felt) => format!("0x{:064x}", felt.unwrap_or_default()),
            Primitive::Felt252(felt) => format!("0x{:064x}", felt.unwrap_or_default()),
            Primitive::U128(u128) => format!("0x{:064x}", u128.unwrap_or_default()),
            Primitive::U64(u64) => format!("0x{:064x}", u64.unwrap_or_default()),
            Primitive::EthAddress(felt) => format!("0x{:064x}", felt.unwrap_or_default()),
            Primitive::U256(u256) => format!("0x{:064x}", u256.unwrap_or_default()),
        }
    }

    pub fn from_sql_value(&mut self, value: &str) -> Result<(), PrimitiveError> {
        match self {
            // Integers - parse directly
            Primitive::I8(ref mut inner) => {
                *inner = Some(
                    value
                        .parse()
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::I16(ref mut inner) => {
                *inner = Some(
                    value
                        .parse()
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::I32(ref mut inner) => {
                *inner = Some(
                    value
                        .parse()
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::I64(ref mut inner) => {
                *inner = Some(
                    value
                        .parse()
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::U8(ref mut inner) => {
                *inner = Some(
                    value
                        .parse()
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::U16(ref mut inner) => {
                *inner = Some(
                    value
                        .parse()
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::U32(ref mut inner) => {
                *inner = Some(
                    value
                        .parse()
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::Bool(ref mut inner) => {
                let int_val: i32 = value
                    .parse()
                    .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(int_val != 0);
            }

            // Hex strings - need to parse hex
            Primitive::I128(ref mut inner) => {
                let hex_str = value
                    .strip_prefix("0x")
                    .ok_or_else(|| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(
                    i128::from_str_radix(hex_str, 16)
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::U64(ref mut inner) => {
                let hex_str = value
                    .strip_prefix("0x")
                    .ok_or_else(|| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(
                    u64::from_str_radix(hex_str, 16)
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::U128(ref mut inner) => {
                let hex_str = value
                    .strip_prefix("0x")
                    .ok_or_else(|| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(
                    u128::from_str_radix(hex_str, 16)
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::U256(ref mut inner) => {
                let hex_str = value
                    .strip_prefix("0x")
                    .ok_or_else(|| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(U256::from_be_hex(hex_str));
            }
            Primitive::ContractAddress(ref mut inner) => {
                let hex_str = value
                    .strip_prefix("0x")
                    .ok_or_else(|| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(
                    Felt::from_hex(hex_str)
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::ClassHash(ref mut inner) => {
                let hex_str = value
                    .strip_prefix("0x")
                    .ok_or_else(|| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(
                    Felt::from_hex(hex_str)
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::Felt252(ref mut inner) => {
                let hex_str = value
                    .strip_prefix("0x")
                    .ok_or_else(|| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(
                    Felt::from_hex(hex_str)
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
            Primitive::EthAddress(ref mut inner) => {
                let hex_str = value
                    .strip_prefix("0x")
                    .ok_or_else(|| PrimitiveError::InvalidSqlValue(value.to_string()))?;
                *inner = Some(
                    Felt::from_hex(hex_str)
                        .map_err(|_| PrimitiveError::InvalidSqlValue(value.to_string()))?,
                );
            }
        }
        Ok(())
    }

    pub fn deserialize(&mut self, felts: &mut Vec<Felt>) -> Result<(), PrimitiveError> {
        if felts.is_empty() {
            return Err(PrimitiveError::MissingFieldElement);
        }

        match self {
            Primitive::I8(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(try_from_felt::<i8>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i8>(), value: felt }
                })?);
            }

            Primitive::I16(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(try_from_felt::<i16>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i16>(), value: felt }
                })?);
            }

            Primitive::I32(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(try_from_felt::<i32>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i32>(), value: felt }
                })?);
            }

            Primitive::I64(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(try_from_felt::<i64>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i64>(), value: felt }
                })?);
            }

            Primitive::I128(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(try_from_felt::<i128>(felt).map_err(|_| {
                    PrimitiveError::ValueOutOfRange { r#type: type_name::<i128>(), value: felt }
                })?);
            }

            Primitive::U8(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(felt.to_u8().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u8>(),
                    value: felt,
                })?);
            }

            Primitive::U16(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(felt.to_u16().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u16>(),
                    value: felt,
                })?);
            }

            Primitive::U32(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(felt.to_u32().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u32>(),
                    value: felt,
                })?);
            }

            Primitive::U64(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(felt.to_u64().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u64>(),
                    value: felt,
                })?);
            }

            Primitive::U128(ref mut value) => {
                let felt = felts.remove(0);
                *value = Some(felt.to_u128().ok_or_else(|| PrimitiveError::ValueOutOfRange {
                    r#type: type_name::<u128>(),
                    value: felt,
                })?);
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
                *value = Some(U256::from_be_bytes(bytes));
            }

            Primitive::Bool(ref mut value) => {
                let raw = felts.remove(0);
                *value = Some(raw == Felt::ONE);
            }

            Primitive::ContractAddress(ref mut value) => {
                *value = Some(felts.remove(0));
            }

            Primitive::ClassHash(ref mut value) => {
                *value = Some(felts.remove(0));
            }

            Primitive::Felt252(ref mut value) => {
                *value = Some(felts.remove(0));
            }

            Primitive::EthAddress(ref mut value) => {
                *value = Some(felts.remove(0));
            }
        }

        Ok(())
    }

    pub fn serialize(&self) -> Result<Vec<Felt>, PrimitiveError> {
        match self {
            Primitive::I8(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::I16(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::I32(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::I64(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::I128(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U8(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U16(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U32(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U64(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U128(value) => value
                .map(|v| Ok(vec![Felt::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U256(value) => value
                .map(|v| {
                    let bytes: [u8; 32] = v.to_be_bytes();
                    let value0_slice = &bytes[16..];
                    let value1_slice = &bytes[..16];
                    let mut value0_array = [0u8; 32];
                    let mut value1_array = [0u8; 32];
                    value0_array[16..].copy_from_slice(value0_slice);
                    value1_array[16..].copy_from_slice(value1_slice);
                    let value0 = Felt::from_bytes_be(&value0_array);
                    let value1 = Felt::from_bytes_be(&value1_array);
                    Ok(vec![value0, value1])
                })
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::Bool(value) => value
                .map(|v| Ok(vec![if v { Felt::ONE } else { Felt::ZERO }]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::ContractAddress(value) => {
                value.map(|v| Ok(vec![v])).unwrap_or(Err(PrimitiveError::MissingFieldElement))
            }
            Primitive::ClassHash(value) => {
                value.map(|v| Ok(vec![v])).unwrap_or(Err(PrimitiveError::MissingFieldElement))
            }
            Primitive::Felt252(value) => {
                value.map(|v| Ok(vec![v])).unwrap_or(Err(PrimitiveError::MissingFieldElement))
            }
            Primitive::EthAddress(value) => {
                value.map(|v| Ok(vec![v])).unwrap_or(Err(PrimitiveError::MissingFieldElement))
            }
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
        let primitive = Primitive::U256(Some(U256::from_be_hex(
            "aaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbccccccccccccccccdddddddddddddddd",
        )));
        let sql_value = primitive.to_sql_value();
        let serialized = primitive.serialize().unwrap();

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
        let mut primitive = Primitive::I8(None);
        primitive.set_i8(Some(-1i8)).unwrap();
        assert_eq!(primitive.as_i8(), Some(-1i8));
        let mut primitive = Primitive::I16(None);
        primitive.set_i16(Some(-1i16)).unwrap();
        assert_eq!(primitive.as_i16(), Some(-1i16));
        let mut primitive = Primitive::I32(None);
        primitive.set_i32(Some(-1i32)).unwrap();
        assert_eq!(primitive.as_i32(), Some(-1i32));
        let mut primitive = Primitive::I64(None);
        primitive.set_i64(Some(-1i64)).unwrap();
        assert_eq!(primitive.as_i64(), Some(-1i64));
        let mut primitive = Primitive::I128(None);
        primitive.set_i128(Some(-1i128)).unwrap();
        assert_eq!(primitive.as_i128(), Some(-1i128));
        let mut primitive = Primitive::U8(None);
        primitive.set_u8(Some(1u8)).unwrap();
        assert_eq!(primitive.as_u8(), Some(1u8));
        let mut primitive = Primitive::U16(None);
        primitive.set_u16(Some(1u16)).unwrap();
        assert_eq!(primitive.as_u16(), Some(1u16));
        let mut primitive = Primitive::U32(None);
        primitive.set_u32(Some(1u32)).unwrap();
        assert_eq!(primitive.as_u32(), Some(1u32));
        let mut primitive = Primitive::U64(None);
        primitive.set_u64(Some(1u64)).unwrap();
        assert_eq!(primitive.as_u64(), Some(1u64));
        let mut primitive = Primitive::U128(None);
        primitive.set_u128(Some(1u128)).unwrap();
        assert_eq!(primitive.as_u128(), Some(1u128));
        let mut primitive = Primitive::U256(None);
        primitive.set_u256(Some(U256::from(1u128))).unwrap();
        assert_eq!(primitive.as_u256(), Some(U256::from(1u128)));
        let mut primitive = Primitive::Bool(None);
        primitive.set_bool(Some(true)).unwrap();
        assert_eq!(primitive.as_bool(), Some(true));
        let mut primitive = Primitive::Felt252(None);
        primitive.set_felt252(Some(Felt::from(1u128))).unwrap();
        assert_eq!(primitive.as_felt252(), Some(Felt::from(1u128)));
        let mut primitive = Primitive::ClassHash(None);
        primitive.set_class_hash(Some(Felt::from(1u128))).unwrap();
        assert_eq!(primitive.as_class_hash(), Some(Felt::from(1u128)));
        let mut primitive = Primitive::ContractAddress(None);
        primitive.set_contract_address(Some(Felt::from(1u128))).unwrap();
        assert_eq!(primitive.as_contract_address(), Some(Felt::from(1u128)));
        let mut primitive = Primitive::EthAddress(None);
        primitive.set_eth_address(Some(Felt::from(1u128))).unwrap();
        assert_eq!(primitive.as_eth_address(), Some(Felt::from(1u128)));
    }

    #[test]
    fn test_primitive_deserialization() {
        let test_cases = vec![
            (vec![Felt::from(-42i8)], Primitive::I8(Some(-42))),
            (vec![Felt::from(-1000i16)], Primitive::I16(Some(-1000))),
            (vec![Felt::from(-100000i32)], Primitive::I32(Some(-100000))),
            (vec![Felt::from(-1000000000i64)], Primitive::I64(Some(-1000000000))),
            (
                vec![Felt::from(-1000000000000000000i128)],
                Primitive::I128(Some(-1000000000000000000)),
            ),
            (vec![Felt::from(42u8)], Primitive::U8(Some(42))),
            (vec![Felt::from(1000u16)], Primitive::U16(Some(1000))),
            (vec![Felt::from(100000u32)], Primitive::U32(Some(100000))),
            (vec![Felt::from(1000000000u64)], Primitive::U64(Some(1000000000))),
            (vec![Felt::from(1000000000000000000u128)], Primitive::U128(Some(1000000000000000000))),
            (vec![Felt::from(1u8)], Primitive::Bool(Some(true))),
            (vec![Felt::from(123456789u128)], Primitive::Felt252(Some(Felt::from(123456789)))),
            (vec![Felt::from(987654321u128)], Primitive::ClassHash(Some(Felt::from(987654321)))),
            (
                vec![Felt::from(123456789u128)],
                Primitive::ContractAddress(Some(Felt::from(123456789))),
            ),
            (vec![Felt::from(123456789u128)], Primitive::EthAddress(Some(Felt::from(123456789)))),
        ];

        for (serialized, expected) in test_cases {
            let mut to_deser = expected;
            to_deser.deserialize(&mut serialized.clone()).unwrap();
            assert_eq!(to_deser, expected);
        }
    }

    #[test]
    fn test_sql_value_round_trip() {
        let test_cases = vec![
            Primitive::I8(Some(-42)),
            Primitive::I16(Some(-1000)),
            Primitive::I32(Some(-100000)),
            Primitive::I64(Some(-1000000000)),
            Primitive::I128(Some(-1000000000000000000)),
            Primitive::U8(Some(42)),
            Primitive::U16(Some(1000)),
            Primitive::U32(Some(100000)),
            Primitive::U64(Some(1000000000)),
            Primitive::U128(Some(1000000000000000000)),
            Primitive::U256(Some(U256::from_be_hex(
                "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            ))),
            Primitive::Bool(Some(true)),
            Primitive::Bool(Some(false)),
            Primitive::Felt252(Some(Felt::from(123456789))),
            Primitive::ClassHash(Some(Felt::from(987654321))),
            Primitive::ContractAddress(Some(Felt::from(123456789))),
            Primitive::EthAddress(Some(Felt::from(123456789))),
        ];

        for original in test_cases {
            // Convert to SQL value
            let sql_value = original.to_sql_value();

            // Create empty primitive of same type
            let mut parsed = match original {
                Primitive::I8(_) => Primitive::I8(None),
                Primitive::I16(_) => Primitive::I16(None),
                Primitive::I32(_) => Primitive::I32(None),
                Primitive::I64(_) => Primitive::I64(None),
                Primitive::I128(_) => Primitive::I128(None),
                Primitive::U8(_) => Primitive::U8(None),
                Primitive::U16(_) => Primitive::U16(None),
                Primitive::U32(_) => Primitive::U32(None),
                Primitive::U64(_) => Primitive::U64(None),
                Primitive::U128(_) => Primitive::U128(None),
                Primitive::U256(_) => Primitive::U256(None),
                Primitive::Bool(_) => Primitive::Bool(None),
                Primitive::Felt252(_) => Primitive::Felt252(None),
                Primitive::ClassHash(_) => Primitive::ClassHash(None),
                Primitive::ContractAddress(_) => Primitive::ContractAddress(None),
                Primitive::EthAddress(_) => Primitive::EthAddress(None),
            };

            // Parse back from SQL value
            parsed.from_sql_value(&sql_value).unwrap();

            // Should match original
            assert_eq!(parsed, original, "Round trip failed for primitive: {:?}", original);
        }
    }
}
