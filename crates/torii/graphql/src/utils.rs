use async_graphql::{Result, Value};

use crate::error::ParseError;
use crate::types::ValueMapping;

pub trait ParseIndexMap: Sized {
    fn parse(indexmap: &ValueMapping, input: &str) -> Result<Self, ParseError>;
}

impl ParseIndexMap for u64 {
    fn parse(indexmap: &ValueMapping, input: &str) -> Result<Self, ParseError> {
        let value = indexmap.get(input).ok_or_else(|| ParseError::NotFound(input.to_string()))?;
        match value {
            Value::Number(n) => return Ok(n.as_u64().unwrap()),
            _ => return Err(ParseError::NotNumber(input.to_string()))
        }
    }
}

impl ParseIndexMap for String {
    fn parse(indexmap: &ValueMapping, input: &str) -> Result<Self, ParseError> {
        let value = indexmap.get(input).ok_or_else(|| ParseError::NotFound(input.to_string()))?;
        match value {
            Value::String(s) => return Ok(s.to_string()),
            _ => return Err(ParseError::NotString(input.to_string()))
        }
    }
}

impl ParseIndexMap for Vec<String> {
    fn parse(indexmap: &ValueMapping, input: &str) -> Result<Self, ParseError> {
        let value = indexmap.get(input).ok_or_else(|| ParseError::NotFound(input.to_string()))?;
        match value {
            Value::List(list) => {
                Ok(list.iter().map(|s| s.to_string().trim_matches('"').to_string()).collect())
            },
            _ => return Err(ParseError::NotList(input.to_string()))
        }
        
    }
}