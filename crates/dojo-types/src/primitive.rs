use crypto_bigint::{Encoding, U256};
use serde::{Deserialize, Serialize};
use starknet::core::types::{FieldElement, ValueOutOfRangeError};
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

#[derive(
    AsRefStr, Display, EnumIter, EnumString, Copy, Clone, Debug, Serialize, Deserialize, PartialEq,
)]
#[strum(serialize_all = "lowercase")]
pub enum Primitive {
    U8(Option<u8>),
    U16(Option<u16>),
    U32(Option<u32>),
    U64(Option<u64>),
    U128(Option<u128>),
    U256(Option<U256>),
    USize(Option<u32>),
    Bool(Option<bool>),
    Felt252(Option<FieldElement>),
    #[strum(serialize = "ClassHash")]
    ClassHash(Option<FieldElement>),
    #[strum(serialize = "ContractAddress")]
    ContractAddress(Option<FieldElement>),
}

#[derive(Debug, thiserror::Error)]
pub enum PrimitiveError {
    #[error("Value must have at least one FieldElement")]
    MissingFieldElement,
    #[error("Not enough FieldElements for U256")]
    NotEnoughFieldElements,
    #[error("Unsupported CairoType for SQL formatting")]
    UnsupportedType,
    #[error(transparent)]
    ValueOutOfRange(#[from] ValueOutOfRangeError),
}

#[derive(AsRefStr, Debug, Display, EnumString, PartialEq)]
#[strum(serialize_all = "UPPERCASE")]
pub enum SqlType {
    Integer,
    Text,
}

impl Primitive {
    /// If the `Primitive` is a u8, returns the associated [`u8`]. Returns `None` otherwise.
    pub fn as_u8(&self) -> Option<u8> {
        match self {
            Primitive::U8(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a u16, returns the associated [`u16`]. Returns `None` otherwise.
    pub fn as_u16(&self) -> Option<u16> {
        match self {
            Primitive::U16(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a u32, returns the associated [`u32`]. Returns `None` otherwise.
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Primitive::U32(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a u64, returns the associated [`u64`]. Returns `None` otherwise.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Primitive::U64(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a u128, returns the associated [`u128`]. Returns `None` otherwise.
    pub fn as_u128(&self) -> Option<u128> {
        match self {
            Primitive::U128(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a u256, returns the associated [`U256`]. Returns `None` otherwise.
    pub fn as_u256(&self) -> Option<U256> {
        match self {
            Primitive::U256(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a felt252, returns the associated [`FieldElement`]. Returns `None`
    /// otherwise.
    pub fn as_felt252(&self) -> Option<FieldElement> {
        match self {
            Primitive::Felt252(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a ClassHash, returns the associated [`FieldElement`]. Returns `None`
    /// otherwise.
    pub fn as_class_hash(&self) -> Option<FieldElement> {
        match self {
            Primitive::ClassHash(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a ContractAddress, returns the associated [`FieldElement`]. Returns
    /// `None` otherwise.
    pub fn as_contract_address(&self) -> Option<FieldElement> {
        match self {
            Primitive::ContractAddress(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a usize, returns the associated [`u32`]. Returns `None` otherwise.
    pub fn as_usize(&self) -> Option<u32> {
        match self {
            Primitive::USize(value) => *value,
            _ => None,
        }
    }

    /// If the `Primitive` is a bool, returns the associated [`bool`]. Returns `None` otherwise.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Primitive::Bool(value) => *value,
            _ => None,
        }
    }

    pub fn to_sql_type(&self) -> SqlType {
        match self {
            Primitive::U8(_)
            | Primitive::U16(_)
            | Primitive::U32(_)
            | Primitive::U64(_)
            | Primitive::USize(_)
            | Primitive::Bool(_) => SqlType::Integer,
            Primitive::U128(_)
            | Primitive::U256(_)
            | Primitive::ContractAddress(_)
            | Primitive::ClassHash(_)
            | Primitive::Felt252(_) => SqlType::Text,
        }
    }

    pub fn to_sql_value(&self) -> Result<String, PrimitiveError> {
        let value = self.serialize()?;

        if value.is_empty() {
            return Err(PrimitiveError::MissingFieldElement);
        }

        match self {
            Primitive::U8(_)
            | Primitive::U16(_)
            | Primitive::U32(_)
            | Primitive::U64(_)
            | Primitive::USize(_)
            | Primitive::Bool(_) => Ok(format!("'{}'", value[0])),

            Primitive::U128(_)
            | Primitive::ContractAddress(_)
            | Primitive::ClassHash(_)
            | Primitive::Felt252(_) => Ok(format!("'0x{:064x}'", value[0])),

            Primitive::U256(_) => {
                if value.len() < 2 {
                    Err(PrimitiveError::NotEnoughFieldElements)
                } else {
                    let mut buffer = [0u8; 32];
                    let value0_bytes = value[0].to_bytes_be();
                    let value1_bytes = value[1].to_bytes_be();
                    buffer[16..].copy_from_slice(&value0_bytes[16..]);
                    buffer[..16].copy_from_slice(&value1_bytes[16..]);
                    Ok(format!("'0x{}'", hex::encode(buffer)))
                }
            }
        }
    }

    pub fn deserialize(&mut self, felts: &mut Vec<FieldElement>) -> Result<(), PrimitiveError> {
        if felts.is_empty() {
            return Err(PrimitiveError::MissingFieldElement);
        }

        match self {
            Primitive::U8(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(PrimitiveError::ValueOutOfRange)?);
                Ok(())
            }
            Primitive::U16(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(PrimitiveError::ValueOutOfRange)?);
                Ok(())
            }
            Primitive::U32(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(PrimitiveError::ValueOutOfRange)?);
                Ok(())
            }
            Primitive::U64(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(PrimitiveError::ValueOutOfRange)?);
                Ok(())
            }
            Primitive::USize(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(PrimitiveError::ValueOutOfRange)?);
                Ok(())
            }
            Primitive::Bool(ref mut value) => {
                let raw = felts.remove(0);
                *value = Some(raw == FieldElement::ONE);
                Ok(())
            }
            Primitive::U128(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(PrimitiveError::ValueOutOfRange)?);
                Ok(())
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
                Ok(())
            }
            Primitive::ContractAddress(ref mut value) => {
                *value = Some(felts.remove(0));
                Ok(())
            }
            Primitive::ClassHash(ref mut value) => {
                *value = Some(felts.remove(0));
                Ok(())
            }
            Primitive::Felt252(ref mut value) => {
                *value = Some(felts.remove(0));
                Ok(())
            }
        }
    }

    pub fn serialize(&self) -> Result<Vec<FieldElement>, PrimitiveError> {
        match self {
            Primitive::U8(value) => value
                .map(|v| Ok(vec![FieldElement::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U16(value) => value
                .map(|v| Ok(vec![FieldElement::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U32(value) => value
                .map(|v| Ok(vec![FieldElement::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U64(value) => value
                .map(|v| Ok(vec![FieldElement::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::USize(value) => value
                .map(|v| Ok(vec![FieldElement::from(v)]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::Bool(value) => value
                .map(|v| Ok(vec![if v { FieldElement::ONE } else { FieldElement::ZERO }]))
                .unwrap_or(Err(PrimitiveError::MissingFieldElement)),
            Primitive::U128(value) => value
                .map(|v| Ok(vec![FieldElement::from(v)]))
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
                    let value0 = FieldElement::from_bytes_be(&value0_array).unwrap();
                    let value1 = FieldElement::from_bytes_be(&value1_array).unwrap();
                    Ok(vec![value0, value1])
                })
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
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crypto_bigint::U256;
    use starknet::core::types::FieldElement;

    use super::Primitive;

    #[test]
    fn test_u256() {
        let primitive = Primitive::U256(Some(U256::from_be_hex(
            "aaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbccccccccccccccccdddddddddddddddd",
        )));
        let sql_value = primitive.to_sql_value().unwrap();
        let serialized = primitive.serialize().unwrap();

        let mut deserialized = primitive;
        deserialized.deserialize(&mut serialized.clone()).unwrap();

        assert_eq!(
            sql_value,
            "'0xaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbccccccccccccccccdddddddddddddddd'"
        );
        assert_eq!(
            serialized,
            vec![
                FieldElement::from_str("0xccccccccccccccccdddddddddddddddd").unwrap(),
                FieldElement::from_str("0xaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbb").unwrap()
            ]
        );
        assert_eq!(deserialized, primitive)
    }

    #[test]
    fn as_inner_value() {
        let primitive = Primitive::U8(Some(1u8));
        assert_eq!(primitive.as_u8(), Some(1u8));
        let primitive = Primitive::U16(Some(1u16));
        assert_eq!(primitive.as_u16(), Some(1u16));
        let primitive = Primitive::U32(Some(1u32));
        assert_eq!(primitive.as_u32(), Some(1u32));
        let primitive = Primitive::U64(Some(1u64));
        assert_eq!(primitive.as_u64(), Some(1u64));
        let primitive = Primitive::U128(Some(1u128));
        assert_eq!(primitive.as_u128(), Some(1u128));
        let primitive = Primitive::U256(Some(U256::from(1u128)));
        assert_eq!(primitive.as_u256(), Some(U256::from(1u128)));
        let primitive = Primitive::USize(Some(1u32));
        assert_eq!(primitive.as_usize(), Some(1u32));
        let primitive = Primitive::Bool(Some(true));
        assert_eq!(primitive.as_bool(), Some(true));
        let primitive = Primitive::Felt252(Some(FieldElement::from(1u128)));
        assert_eq!(primitive.as_felt252(), Some(FieldElement::from(1u128)));
        let primitive = Primitive::ClassHash(Some(FieldElement::from(1u128)));
        assert_eq!(primitive.as_class_hash(), Some(FieldElement::from(1u128)));
        let primitive = Primitive::ContractAddress(Some(FieldElement::from(1u128)));
        assert_eq!(primitive.as_contract_address(), Some(FieldElement::from(1u128)));
    }
}
