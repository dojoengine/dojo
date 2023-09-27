use std::str::FromStr;

use starknet::core::types::FieldElement;

pub enum CairoType {
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    USize,
    Bool,
    ContractAddress,
    ClassHash,
    Felt252,
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

impl FromStr for CairoType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "u8" => Ok(CairoType::U8),
            "u16" => Ok(CairoType::U16),
            "u32" => Ok(CairoType::U32),
            "u64" => Ok(CairoType::U64),
            "u128" => Ok(CairoType::U128),
            "u256" => Ok(CairoType::U256),
            "usize" => Ok(CairoType::USize),
            "bool" => Ok(CairoType::Bool),
            "ContractAddress" => Ok(CairoType::ContractAddress),
            "ClassHash" => Ok(CairoType::ClassHash),
            "felt252" => Ok(CairoType::Felt252),
            _ => Err(()),
        }
    }
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
