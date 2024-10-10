use cainome::parser::tokens::Composite;

pub(crate) mod r#enum;
pub(crate) mod function;
pub(crate) mod interface;
pub(crate) mod schema;

pub(crate) struct JsType(String);
impl From<&str> for JsType {
    fn from(value: &str) -> Self {
        match value {
            "felt252" => JsType("number".to_owned()),
            "ContractAddress" => JsType("string".to_owned()),
            "ByteArray" => JsType("string".to_owned()),
            "u8" => JsType("number".to_owned()),
            "u16" => JsType("number".to_owned()),
            "u32" => JsType("number".to_owned()),
            "u64" => JsType("number".to_owned()),
            "u128" => JsType("number".to_owned()),
            "u256" => JsType("number".to_owned()),
            _ => JsType(value.to_owned()),
        }
    }
}

pub(crate) struct JsDefaultValue(String);
impl From<&str> for JsDefaultValue {
    fn from(value: &str) -> Self {
        match value {
            "felt252" => JsDefaultValue("0".to_string()),
            "ContractAddress" => JsDefaultValue("\"\"".to_string()),
            "ByteArray" => JsDefaultValue("\"\"".to_string()),
            "u8" => JsDefaultValue("0".to_string()),
            "u16" => JsDefaultValue("0".to_string()),
            "u32" => JsDefaultValue("0".to_string()),
            "u64" => JsDefaultValue("0".to_string()),
            "u128" => JsDefaultValue("0".to_string()),
            "u256" => JsDefaultValue("0".to_string()),
            _ => JsDefaultValue(value.to_string()),
        }
    }
}
impl From<&Composite> for JsDefaultValue {
    fn from(value: &Composite) -> Self {
        match value.r#type {
            cainome::parser::tokens::CompositeType::Enum => {
                JsDefaultValue(format!("{}.{}", value.type_name(), value.inners[0].name))
            }
            _ => JsDefaultValue::from(value.type_name().as_str()),
        }
    }
}

impl std::fmt::Display for JsDefaultValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
