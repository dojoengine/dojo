use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum ScalarType {
    U8,
    U16,
    U32,
    U64,
    U128,
    U256,
    USize,
    Bool,
    Cursor,
    Address,
    DateTime,
    Felt252,
}

impl fmt::Display for ScalarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ScalarType::U8 => write!(f, "u8"),
            ScalarType::U16 => write!(f, "u16"),
            ScalarType::U32 => write!(f, "u32"),
            ScalarType::U64 => write!(f, "u64"),
            ScalarType::U128 => write!(f, "u128"),
            ScalarType::U256 => write!(f, "u256"),
            ScalarType::USize => write!(f, "usize"),
            ScalarType::Bool => write!(f, "bool"),
            ScalarType::Cursor => write!(f, "Cursor"),
            ScalarType::Address => write!(f, "ContractAddress"),
            ScalarType::DateTime => write!(f, "DateTime"),
            ScalarType::Felt252 => write!(f, "felt252"),
        }
    }
}

impl ScalarType {
    pub fn types() -> HashSet<ScalarType> {
        vec![
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::U128,
            ScalarType::U256,
            ScalarType::USize,
            ScalarType::Bool,
            ScalarType::Cursor,
            ScalarType::Address,
            ScalarType::DateTime,
            ScalarType::Felt252,
        ]
        .into_iter()
        .collect()
    }

    pub fn numeric_types() -> HashSet<ScalarType> {
        vec![
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::USize,
            ScalarType::Bool,
        ]
        .into_iter()
        .collect()
    }

    // u128 and u256 are non numeric here due to
    // sqlite constraint on integer columns
    pub fn _non_numeric_types() -> HashSet<ScalarType> {
        vec![
            ScalarType::U128,
            ScalarType::U256,
            ScalarType::Cursor,
            ScalarType::Address,
            ScalarType::DateTime,
            ScalarType::Felt252,
        ]
        .into_iter()
        .collect()
    }

    pub fn is_numeric_type(&self) -> bool {
        ScalarType::numeric_types().contains(self)
    }

    pub fn from_str<S: AsRef<str>>(s: S) -> Result<ScalarType, anyhow::Error> {
        match s.as_ref() {
            "u8" => Ok(ScalarType::U8),
            "u16" => Ok(ScalarType::U16),
            "u32" => Ok(ScalarType::U32),
            "u64" => Ok(ScalarType::U64),
            "u128" => Ok(ScalarType::U128),
            "u256" => Ok(ScalarType::U256),
            "usize" => Ok(ScalarType::USize),
            "bool" => Ok(ScalarType::Bool),
            "Cursor" => Ok(ScalarType::Cursor),
            "ContractAddress" => Ok(ScalarType::Address),
            "DateTime" => Ok(ScalarType::DateTime),
            "felt252" => Ok(ScalarType::Felt252),
            _ => Err(anyhow::anyhow!("Unknown type {}", s.as_ref().to_string())),
        }
    }

    pub fn as_sql_type(&self) -> &'static str {
        if self.is_numeric_type() { "INTEGER" } else { "TEXT" }
    }
}
