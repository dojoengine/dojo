use crypto_bigint::{Encoding, U256};
use serde::{Deserialize, Serialize};
use starknet::core::types::{FieldElement, ValueOutOfRangeError};
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

#[derive(
    AsRefStr, Display, EnumIter, EnumString, Clone, Debug, Serialize, Deserialize, PartialEq,
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

impl Primitive {
    pub fn to_sql_type(&self) -> String {
        match self {
            Primitive::U8(_)
            | Primitive::U16(_)
            | Primitive::U32(_)
            | Primitive::U64(_)
            | Primitive::USize(_)
            | Primitive::Bool(_) => "INTEGER".to_string(),
            Primitive::U128(_)
            | Primitive::U256(_)
            | Primitive::ContractAddress(_)
            | Primitive::ClassHash(_)
            | Primitive::Felt252(_) => "TEXT".to_string(),
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
            | Primitive::Felt252(_) => Ok(format!("'{:0>64x}'", value[0])),

            Primitive::U256(_) => {
                if value.len() < 2 {
                    Err(PrimitiveError::NotEnoughFieldElements)
                } else {
                    let mut buffer = [0u8; 32];
                    let value0_bytes = value[0].to_bytes_be();
                    let value1_bytes = value[1].to_bytes_be();
                    buffer[..16].copy_from_slice(&value0_bytes);
                    buffer[16..].copy_from_slice(&value1_bytes);
                    Ok(format!("'{}'", hex::encode(buffer)))
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
                bytes[..16].copy_from_slice(&value0_bytes);
                bytes[16..].copy_from_slice(&value1_bytes);
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
                    let value0_slice = &bytes[..16];
                    let value1_slice = &bytes[16..];
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
