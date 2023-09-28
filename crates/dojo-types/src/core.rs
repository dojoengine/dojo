use starknet::core::types::FieldElement;
use strum_macros::{AsRefStr, Display, EnumIter, EnumString};

#[derive(AsRefStr, Display, EnumIter, EnumString, Debug)]
#[strum(serialize_all = "lowercase")]
pub enum CairoType {
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    USize,
    Bool,
    Felt252,
    #[strum(serialize = "ClassHash")]
    ClassHash,
    #[strum(serialize = "ContractAddress")]
    ContractAddress,
}

#[derive(Debug, thiserror::Error)]
pub enum CairoTypeError {
    #[error("Value must have at least one FieldElement")]
    MissingFieldElement,
    #[error("Not enough FieldElements for U256")]
    NotEnoughFieldElements,
    #[error("Unsupported CairoType for SQL formatting")]
    UnsupportedType,
}

impl CairoType {
    pub fn to_sql_type(&self) -> String {
        match self {
            CairoType::U8
            | CairoType::U16
            | CairoType::U32
            | CairoType::U64
            | CairoType::USize
            | CairoType::Bool => "INTEGER".to_string(),
            CairoType::U128
            | CairoType::U256
            | CairoType::ContractAddress
            | CairoType::ClassHash
            | CairoType::Felt252 => "TEXT".to_string(),
        }
    }

    pub fn format_for_sql(&self, value: Vec<&FieldElement>) -> Result<String, CairoTypeError> {
        if value.is_empty() {
            return Err(CairoTypeError::MissingFieldElement);
        }

        match self {
            CairoType::U8
            | CairoType::U16
            | CairoType::U32
            | CairoType::U64
            | CairoType::USize
            | CairoType::Bool => Ok(format!(", '{}'", value[0])),
            CairoType::U128
            | CairoType::ContractAddress
            | CairoType::ClassHash
            | CairoType::Felt252 => Ok(format!(", '{:0>64x}'", value[0])),
            CairoType::U256 => {
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
}
