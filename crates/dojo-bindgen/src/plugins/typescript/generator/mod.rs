pub(crate) mod r#enum;
pub(crate) mod function;
pub(crate) mod interface;
pub(crate) mod schema;

pub(crate) struct JsType(String);
impl From<&str> for JsType {
    fn from(value: &str) -> Self {
        match value {
            "felt252" => JsType("number".to_string()),
            "ContractAddress" => JsType("string".to_string()),
            "ByteArray" => JsType("string".to_string()),
            "u8" => JsType("number".to_string()),
            "u16" => JsType("number".to_string()),
            "u32" => JsType("number".to_string()),
            "u64" => JsType("number".to_string()),
            "u128" => JsType("number".to_string()),
            "u256" => JsType("number".to_string()),
            _ => JsType(value.to_string()),
        }
    }
}

