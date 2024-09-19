use std::str::FromStr;

use async_graphql::{Result, Value};
use convert_case::{Case, Casing};
use starknet_crypto::Felt;

use crate::error::ExtractError;
use crate::types::ValueMapping;

pub trait ExtractFromIndexMap: Sized {
    fn extract(indexmap: &ValueMapping, input: &str) -> Result<Self, ExtractError>;
}

impl ExtractFromIndexMap for u64 {
    fn extract(indexmap: &ValueMapping, input: &str) -> Result<Self, ExtractError> {
        let value = indexmap.get(input).ok_or_else(|| ExtractError::NotFound(input.to_string()))?;
        match value {
            Value::Number(n) => Ok(n.as_u64().unwrap()),
            _ => Err(ExtractError::NotNumber(input.to_string())),
        }
    }
}

impl ExtractFromIndexMap for String {
    fn extract(indexmap: &ValueMapping, input: &str) -> Result<Self, ExtractError> {
        let value = indexmap.get(input).ok_or_else(|| ExtractError::NotFound(input.to_string()))?;
        match value {
            Value::String(s) => Ok(s.to_string()),
            _ => Err(ExtractError::NotString(input.to_string())),
        }
    }
}

impl ExtractFromIndexMap for Felt {
    fn extract(indexmap: &ValueMapping, input: &str) -> Result<Self, ExtractError> {
        let value = indexmap.get(input).ok_or_else(|| ExtractError::NotFound(input.to_string()))?;
        match value {
            Value::String(s) => {
                Ok(Felt::from_str(s).map_err(|_| ExtractError::NotFelt(input.to_string()))?)
            }
            _ => Err(ExtractError::NotString(input.to_string())),
        }
    }
}

impl ExtractFromIndexMap for Vec<String> {
    fn extract(indexmap: &ValueMapping, input: &str) -> Result<Self, ExtractError> {
        let value = indexmap.get(input).ok_or_else(|| ExtractError::NotFound(input.to_string()))?;
        match value {
            Value::List(list) => {
                Ok(list.iter().map(|s| s.to_string().trim_matches('"').to_string()).collect())
            }
            _ => Err(ExtractError::NotList(input.to_string())),
        }
    }
}

pub fn extract<T: ExtractFromIndexMap>(
    values: &ValueMapping,
    key: &str,
) -> Result<T, ExtractError> {
    T::extract(values, key)
}

pub fn field_name_from_names(namespace: &str, model_name: &str) -> String {
    format!("{}{}", namespace.to_case(Case::Camel), model_name.to_case(Case::Pascal))
}

pub fn type_name_from_names(namespace: &str, model_name: &str) -> String {
    format!("{}_{}", namespace, model_name)
}

pub fn struct_name_from_names(namespace: &str, model_name: &str) -> String {
    format!("{}-{}", namespace, model_name)
}
