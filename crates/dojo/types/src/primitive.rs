use std::any::type_name;

use crypto_bigint::{Encoding, U256};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
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
    #[error("Invalid JSON value format for type {r#type}: {value}")]
    InvalidJsonValue { r#type: &'static str, value: String },
    #[error("JSON number out of range for {r#type}: {value}")]
    JsonNumberOutOfRange { r#type: &'static str, value: String },
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
            // SQLite integers (signed 64-bit)
            Primitive::I8(v) => v.unwrap_or_default().to_string(),
            Primitive::I16(v) => v.unwrap_or_default().to_string(),
            Primitive::I32(v) => v.unwrap_or_default().to_string(),
            Primitive::I64(v) => v.unwrap_or_default().to_string(),
            Primitive::U8(v) => v.unwrap_or_default().to_string(),
            Primitive::U16(v) => v.unwrap_or_default().to_string(),
            Primitive::U32(v) => v.unwrap_or_default().to_string(),
            Primitive::Bool(v) => (v.unwrap_or_default() as i32).to_string(),

            // Large integers and addresses as hex strings (for SQLite TEXT)
            Primitive::I128(v) => format!("0x{:032x}", v.unwrap_or_default()),
            Primitive::U64(v) => format!("0x{:016x}", v.unwrap_or_default()),
            Primitive::U128(v) => format!("0x{:032x}", v.unwrap_or_default()),
            Primitive::U256(v) => format!("0x{:064x}", v.unwrap_or_default()),
            Primitive::ContractAddress(v) => format!("0x{:064x}", v.unwrap_or_default()),
            Primitive::ClassHash(v) => format!("0x{:064x}", v.unwrap_or_default()),
            Primitive::Felt252(v) => format!("0x{:064x}", v.unwrap_or_default()),
            Primitive::EthAddress(v) => format!("0x{:040x}", v.unwrap_or_default()),
        }
    }

    pub fn from_sql_value(&mut self, value: &str) -> Result<(), PrimitiveError> {
        match self {
            // SQLite integers - parse directly
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

            // Hex strings - need to parse hex (stored as TEXT in SQLite)
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

    /// Convert to JSON Value with proper type representation
    pub fn to_json_value(&self) -> Result<JsonValue, PrimitiveError> {
        match self {
            // Small integers that fit in JSON Number safely (up to 2^53 - 1)
            Primitive::I8(Some(v)) => Ok(json!(*v)),
            Primitive::I16(Some(v)) => Ok(json!(*v)),
            Primitive::I32(Some(v)) => Ok(json!(*v)),
            Primitive::U8(Some(v)) => Ok(json!(*v)),
            Primitive::U16(Some(v)) => Ok(json!(*v)),
            Primitive::U32(Some(v)) => Ok(json!(*v)),
            Primitive::Bool(Some(v)) => Ok(json!(*v)),

            // Large integers as decimal strings for JSON
            Primitive::I64(Some(v)) => Ok(json!(v.to_string())),
            Primitive::I128(Some(v)) => Ok(json!(v.to_string())),
            Primitive::U64(Some(v)) => Ok(json!(v.to_string())),
            Primitive::U128(Some(v)) => Ok(json!(v.to_string())),

            // U256 as hex string due to its extremely large range
            Primitive::U256(Some(v)) => Ok(json!(format!("0x{:064x}", v))),

            // Blockchain-specific types as hex strings
            Primitive::ContractAddress(Some(v)) => Ok(json!(format!("0x{:064x}", v))),
            Primitive::ClassHash(Some(v)) => Ok(json!(format!("0x{:064x}", v))),
            Primitive::Felt252(Some(v)) => Ok(json!(format!("0x{:064x}", v))),
            Primitive::EthAddress(Some(v)) => Ok(json!(format!("0x{:040x}", v))),

            // None values
            _ => Err(PrimitiveError::MissingFieldElement),
        }
    }

    /// Parse from JSON Value with proper type validation
    pub fn from_json_value(&mut self, value: JsonValue) -> Result<(), PrimitiveError> {
        match (self, value) {
            // Boolean handling
            (Primitive::Bool(ref mut inner), JsonValue::Bool(b)) => {
                *inner = Some(b);
            }
            (Primitive::Bool(ref mut inner), JsonValue::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    *inner = Some(i != 0);
                } else {
                    return Err(PrimitiveError::InvalidJsonValue {
                        r#type: "Bool",
                        value: n.to_string(),
                    });
                }
            }

            // Small signed integers from JSON numbers
            (Primitive::I8(ref mut inner), JsonValue::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    if i >= i8::MIN as i64 && i <= i8::MAX as i64 {
                        *inner = Some(i as i8);
                    } else {
                        return Err(PrimitiveError::JsonNumberOutOfRange {
                            r#type: "I8",
                            value: i.to_string(),
                        });
                    }
                } else {
                    return Err(PrimitiveError::InvalidJsonValue {
                        r#type: "I8",
                        value: n.to_string(),
                    });
                }
            }
            (Primitive::I16(ref mut inner), JsonValue::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    if i >= i16::MIN as i64 && i <= i16::MAX as i64 {
                        *inner = Some(i as i16);
                    } else {
                        return Err(PrimitiveError::JsonNumberOutOfRange {
                            r#type: "I16",
                            value: i.to_string(),
                        });
                    }
                } else {
                    return Err(PrimitiveError::InvalidJsonValue {
                        r#type: "I16",
                        value: n.to_string(),
                    });
                }
            }
            (Primitive::I32(ref mut inner), JsonValue::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                        *inner = Some(i as i32);
                    } else {
                        return Err(PrimitiveError::JsonNumberOutOfRange {
                            r#type: "I32",
                            value: i.to_string(),
                        });
                    }
                } else {
                    return Err(PrimitiveError::InvalidJsonValue {
                        r#type: "I32",
                        value: n.to_string(),
                    });
                }
            }

            // Small unsigned integers from JSON numbers
            (Primitive::U8(ref mut inner), JsonValue::Number(n)) => {
                if let Some(u) = n.as_u64() {
                    if u <= u8::MAX as u64 {
                        *inner = Some(u as u8);
                    } else {
                        return Err(PrimitiveError::JsonNumberOutOfRange {
                            r#type: "U8",
                            value: u.to_string(),
                        });
                    }
                } else {
                    return Err(PrimitiveError::InvalidJsonValue {
                        r#type: "U8",
                        value: n.to_string(),
                    });
                }
            }
            (Primitive::U16(ref mut inner), JsonValue::Number(n)) => {
                if let Some(u) = n.as_u64() {
                    if u <= u16::MAX as u64 {
                        *inner = Some(u as u16);
                    } else {
                        return Err(PrimitiveError::JsonNumberOutOfRange {
                            r#type: "U16",
                            value: u.to_string(),
                        });
                    }
                } else {
                    return Err(PrimitiveError::InvalidJsonValue {
                        r#type: "U16",
                        value: n.to_string(),
                    });
                }
            }
            (Primitive::U32(ref mut inner), JsonValue::Number(n)) => {
                if let Some(u) = n.as_u64() {
                    if u <= u32::MAX as u64 {
                        *inner = Some(u as u32);
                    } else {
                        return Err(PrimitiveError::JsonNumberOutOfRange {
                            r#type: "U32",
                            value: u.to_string(),
                        });
                    }
                } else {
                    return Err(PrimitiveError::InvalidJsonValue {
                        r#type: "U32",
                        value: n.to_string(),
                    });
                }
            }

            // Large integers from strings (decimal) or numbers
            (Primitive::I64(ref mut inner), JsonValue::String(s)) => {
                *inner =
                    Some(s.parse().map_err(|_| PrimitiveError::InvalidJsonValue {
                        r#type: "I64",
                        value: s,
                    })?);
            }
            (Primitive::I64(ref mut inner), JsonValue::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    *inner = Some(i);
                } else {
                    return Err(PrimitiveError::InvalidJsonValue {
                        r#type: "I64",
                        value: n.to_string(),
                    });
                }
            }

            // String parsing for large integers and addresses
            (primitive, JsonValue::String(s)) => {
                match primitive {
                    Primitive::I128(ref mut inner) => {
                        *inner = Some(s.parse().map_err(|_| PrimitiveError::InvalidJsonValue {
                            r#type: "I128",
                            value: s,
                        })?);
                    }
                    Primitive::U64(ref mut inner) => {
                        *inner = Some(s.parse().map_err(|_| PrimitiveError::InvalidJsonValue {
                            r#type: "U64",
                            value: s,
                        })?);
                    }
                    Primitive::U128(ref mut inner) => {
                        *inner = Some(s.parse().map_err(|_| PrimitiveError::InvalidJsonValue {
                            r#type: "U128",
                            value: s,
                        })?);
                    }
                    Primitive::U256(ref mut inner) => {
                        // U256 should always be hex strings
                        let hex_str = s.strip_prefix("0x").unwrap_or(&s);
                        *inner = Some(U256::from_be_hex(hex_str));
                    }
                    Primitive::ContractAddress(ref mut inner) => {
                        let hex_str = s.strip_prefix("0x").unwrap_or(&s);
                        *inner = Some(Felt::from_hex(hex_str).map_err(|_| {
                            PrimitiveError::InvalidJsonValue { r#type: "ContractAddress", value: s }
                        })?);
                    }
                    Primitive::ClassHash(ref mut inner) => {
                        let hex_str = s.strip_prefix("0x").unwrap_or(&s);
                        *inner = Some(Felt::from_hex(hex_str).map_err(|_| {
                            PrimitiveError::InvalidJsonValue { r#type: "ClassHash", value: s }
                        })?);
                    }
                    Primitive::Felt252(ref mut inner) => {
                        let hex_str = s.strip_prefix("0x").unwrap_or(&s);
                        *inner = Some(Felt::from_hex(hex_str).map_err(|_| {
                            PrimitiveError::InvalidJsonValue { r#type: "Felt252", value: s }
                        })?);
                    }
                    Primitive::EthAddress(ref mut inner) => {
                        let hex_str = s.strip_prefix("0x").unwrap_or(&s);
                        *inner = Some(Felt::from_hex(hex_str).map_err(|_| {
                            PrimitiveError::InvalidJsonValue { r#type: "EthAddress", value: s }
                        })?);
                    }
                    _ => {
                        return Err(PrimitiveError::InvalidJsonValue {
                            r#type: "Unknown",
                            value: s,
                        });
                    }
                }
            }

            _ => {
                return Err(PrimitiveError::TypeMismatch);
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
    use serde_json::json;
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

    #[test]
    fn test_json_value_round_trip() {
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
            // Convert to JSON value
            let json_value = original.to_json_value().unwrap();

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

            // Parse back from JSON value
            parsed.from_json_value(json_value).unwrap();

            // Should match original
            assert_eq!(parsed, original, "JSON round trip failed for primitive: {:?}", original);
        }
    }

    #[test]
    fn test_json_value_types() {
        // Test that small integers are represented as JSON numbers
        let small_int = Primitive::I32(Some(42));
        let json_val = small_int.to_json_value().unwrap();
        assert!(json_val.is_number());
        assert_eq!(json_val.as_i64().unwrap(), 42);

        // Test that large integers are represented as decimal strings
        let large_int = Primitive::U128(Some(u128::MAX));
        let json_val = large_int.to_json_value().unwrap();
        assert!(json_val.is_string());
        assert_eq!(json_val.as_str().unwrap(), u128::MAX.to_string());

        // Test that U256 is always represented as hex string
        let u256_val = Primitive::U256(Some(U256::from(12345u128)));
        let json_val = u256_val.to_json_value().unwrap();
        assert!(json_val.is_string());
        let expected = format!("0x{:064x}", U256::from(12345u128));
        assert_eq!(json_val.as_str().unwrap(), expected);

        // Test boolean representation
        let bool_val = Primitive::Bool(Some(true));
        let json_val = bool_val.to_json_value().unwrap();
        assert!(json_val.is_boolean());
        assert_eq!(json_val.as_bool().unwrap(), true);

        // Test contract address representation
        let addr = Primitive::ContractAddress(Some(Felt::from(0x123456789abcdefu64)));
        let json_val = addr.to_json_value().unwrap();
        assert!(json_val.is_string());
        let expected = format!("0x{:064x}", Felt::from(0x123456789abcdefu64));
        assert_eq!(json_val.as_str().unwrap(), expected);
    }

    #[test]
    fn test_json_parsing_edge_cases() {
        // Test parsing boolean from number
        let mut bool_prim = Primitive::Bool(None);
        bool_prim.from_json_value(json!(1)).unwrap();
        assert_eq!(bool_prim.as_bool(), Some(true));

        bool_prim.from_json_value(json!(0)).unwrap();
        assert_eq!(bool_prim.as_bool(), Some(false));

        // Test parsing decimal strings for large integers
        let mut u128_prim = Primitive::U128(None);
        u128_prim.from_json_value(json!("255")).unwrap();
        assert_eq!(u128_prim.as_u128(), Some(255));

        // Test parsing large decimal numbers
        let mut i128_prim = Primitive::I128(None);
        i128_prim.from_json_value(json!("-170141183460469231731687303715884105728")).unwrap();
        assert_eq!(i128_prim.as_i128(), Some(i128::MIN));

        // Test U256 parsing from hex strings (with and without 0x prefix)
        let mut u256_prim = Primitive::U256(None);
        u256_prim.from_json_value(json!("0x1234567890abcdef")).unwrap();
        assert_eq!(u256_prim.as_u256(), Some(U256::from_be_hex("1234567890abcdef")));

        u256_prim.from_json_value(json!("1234567890abcdef")).unwrap();
        assert_eq!(u256_prim.as_u256(), Some(U256::from_be_hex("1234567890abcdef")));

        // Test range validation for small integers
        let mut i8_prim = Primitive::I8(None);
        assert!(i8_prim.from_json_value(json!(127)).is_ok()); // Valid i8
        assert!(i8_prim.from_json_value(json!(200)).is_err()); // Out of range for i8

        let mut u8_prim = Primitive::U8(None);
        assert!(u8_prim.from_json_value(json!(255)).is_ok()); // Valid u8
        assert!(u8_prim.from_json_value(json!(256)).is_err()); // Out of range for u8
    }

    #[test]
    fn test_json_error_handling() {
        // Test type mismatch errors
        let mut i32_prim = Primitive::I32(None);
        assert!(i32_prim.from_json_value(json!("not_a_number")).is_err());
        assert!(i32_prim.from_json_value(json!(true)).is_err());

        // Test missing field element error
        let none_prim = Primitive::I32(None);
        assert!(none_prim.to_json_value().is_err());

        // Test invalid hex string
        let mut felt_prim = Primitive::Felt252(None);
        assert!(felt_prim.from_json_value(json!("0xgg")).is_err());
    }
}
