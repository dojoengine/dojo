use ethabi::ethereum_types::U256;
use serde::{Deserialize, Serialize};
use starknet::core::types::{FieldElement, ValueOutOfRangeError};
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

#[derive(
    AsRefStr, Display, EnumIter, EnumString, Clone, Debug, Serialize, Deserialize, PartialEq,
)]
#[strum(serialize_all = "lowercase")]
pub enum CairoType {
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
pub enum CairoTypeError {
    #[error("Value must have at least one FieldElement")]
    MissingFieldElement,
    #[error("Not enough FieldElements for U256")]
    NotEnoughFieldElements,
    #[error("Unsupported CairoType for SQL formatting")]
    UnsupportedType,
    #[error(transparent)]
    ValueOutOfRange(#[from] ValueOutOfRangeError),
}

impl CairoType {
    pub fn to_sql_type(&self) -> String {
        match self {
            CairoType::U8(_)
            | CairoType::U16(_)
            | CairoType::U32(_)
            | CairoType::U64(_)
            | CairoType::USize(_)
            | CairoType::Bool(_) => "INTEGER".to_string(),
            CairoType::U128(_)
            | CairoType::U256(_)
            | CairoType::ContractAddress(_)
            | CairoType::ClassHash(_)
            | CairoType::Felt252(_) => "TEXT".to_string(),
        }
    }

    pub fn format_for_sql(&self, value: Vec<&FieldElement>) -> Result<String, CairoTypeError> {
        if value.is_empty() {
            return Err(CairoTypeError::MissingFieldElement);
        }

        match self {
            CairoType::U8(_)
            | CairoType::U16(_)
            | CairoType::U32(_)
            | CairoType::U64(_)
            | CairoType::USize(_)
            | CairoType::Bool(_) => Ok(format!(", '{}'", value[0])),
            CairoType::U128(_)
            | CairoType::ContractAddress(_)
            | CairoType::ClassHash(_)
            | CairoType::Felt252(_) => Ok(format!(", '{:0>64x}'", value[0])),
            CairoType::U256(_) => {
                if value.len() < 2 {
                    Err(CairoTypeError::NotEnoughFieldElements)
                } else {
                    let mut buffer = [0u8; 32];
                    let value0_bytes = value[0].to_bytes_be();
                    let value1_bytes = value[1].to_bytes_be();
                    buffer[..16].copy_from_slice(&value0_bytes);
                    buffer[16..].copy_from_slice(&value1_bytes);
                    Ok(format!(", '{}'", hex::encode(buffer)))
                }
            }
        }
    }

    pub fn set_value_from_felts(
        &mut self,
        felts: &mut Vec<FieldElement>,
    ) -> Result<(), CairoTypeError> {
        if felts.is_empty() {
            return Err(CairoTypeError::MissingFieldElement);
        }

        match self {
            CairoType::U8(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(CairoTypeError::ValueOutOfRange)?);
                Ok(())
            }
            CairoType::U16(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(CairoTypeError::ValueOutOfRange)?);
                Ok(())
            }
            CairoType::U32(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(CairoTypeError::ValueOutOfRange)?);
                Ok(())
            }
            CairoType::U64(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(CairoTypeError::ValueOutOfRange)?);
                Ok(())
            }
            CairoType::USize(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(CairoTypeError::ValueOutOfRange)?);
                Ok(())
            }
            CairoType::Bool(ref mut value) => {
                let raw = felts.remove(0);
                *value = Some(raw == FieldElement::ONE);
                Ok(())
            }
            CairoType::U128(ref mut value) => {
                *value = Some(felts.remove(0).try_into().map_err(CairoTypeError::ValueOutOfRange)?);
                Ok(())
            }
            CairoType::U256(ref mut value) => {
                if felts.len() < 2 {
                    return Err(CairoTypeError::NotEnoughFieldElements);
                }
                let value0 = felts.remove(0);
                let value1 = felts.remove(0);
                let value0_bytes = value0.to_bytes_be();
                let value1_bytes = value1.to_bytes_be();
                let mut bytes = [0u8; 32];
                bytes[..16].copy_from_slice(&value0_bytes);
                bytes[16..].copy_from_slice(&value1_bytes);
                *value = Some(U256::from(bytes));
                Ok(())
            }
            CairoType::ContractAddress(ref mut value) => {
                *value = Some(felts.remove(0));
                Ok(())
            }
            CairoType::ClassHash(ref mut value) => {
                *value = Some(felts.remove(0));
                Ok(())
            }
            CairoType::Felt252(ref mut value) => {
                *value = Some(felts.remove(0));
                Ok(())
            }
        }
    }
}
