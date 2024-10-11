use cainome::parser::tokens::{Composite, Token};

pub(crate) mod r#enum;
pub(crate) mod function;
pub(crate) mod interface;
pub(crate) mod schema;

#[derive(Debug)]
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
            "U256" => JsType("number".to_owned()),
            "bool" => JsType("boolean".to_owned()),
            _ => JsType(value.to_owned()),
        }
    }
}

impl From<&Token> for JsType {
    fn from(value: &Token) -> Self {
        match value {
            Token::Array(a) => JsType::from(format!("Array<{}>", JsType::from(&*a.inner)).as_str()),
            Token::Tuple(t) => JsType::from(
                format!(
                    "[{}]",
                    t.inners
                        .iter()
                        .map(|i| JsType::from(i.type_name().as_str()).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                        .as_str()
                )
                .as_str(),
            ),
            _ => JsType::from(value.type_name().as_str()),
        }
    }
}

impl std::fmt::Display for JsType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug)]
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
            "U256" => JsDefaultValue("0".to_string()),
            "bool" => JsDefaultValue("false".to_string()),
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
            cainome::parser::tokens::CompositeType::Struct => JsDefaultValue(format!(
                "{{ {} }}",
                value
                    .inners
                    .iter()
                    .map(|i| format!("{}: {},", i.name, JsDefaultValue::from(&i.token)))
                    .collect::<Vec<_>>()
                    .join("\n")
            )),
            _ => JsDefaultValue::from(value.type_name().as_str()),
        }
    }
}

impl From<&Token> for JsDefaultValue {
    fn from(value: &Token) -> Self {
        match value {
            Token::Array(a) => {
                JsDefaultValue::from(format!("[{}]", JsDefaultValue::from(&*a.inner)).as_str())
            }
            Token::Tuple(t) => JsDefaultValue::from(
                format!(
                    "[{}]",
                    t.inners
                        .iter()
                        .map(|i| JsDefaultValue::from(i.type_name().as_str()).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                        .as_str()
                )
                .as_str(),
            ),
            Token::Composite(c) => JsDefaultValue::from(c),
            _ => JsDefaultValue::from(value.type_name().as_str()),
        }
    }
}

impl std::fmt::Display for JsDefaultValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use cainome::parser::tokens::{Array, CoreBasic, Token, Tuple};

    use crate::plugins::typescript::generator::{JsDefaultValue, JsType};

    impl PartialEq<JsType> for &str {
        fn eq(&self, other: &JsType) -> bool {
            self == &other.0.as_str()
        }
    }

    impl PartialEq<JsDefaultValue> for &str {
        fn eq(&self, other: &JsDefaultValue) -> bool {
            self == &other.0.as_str()
        }
    }

    #[test]
    fn test_js_type_basics() {
        assert_eq!(
            "number",
            JsType::from(&Token::CoreBasic(CoreBasic {
                type_path: "core::integer::u8".to_owned()
            }))
        );
        assert_eq!(
            "number",
            JsType::from(&Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_owned() }))
        )
    }

    #[test]
    fn test_tuple_type() {
        assert_eq!(
            "[number, number]",
            JsType::from(&Token::Tuple(Tuple {
                type_path: "(core::integer::u8,core::integer::u128)".to_owned(),
                inners: vec![
                    Token::CoreBasic(CoreBasic { type_path: "core::integer::u8".to_owned() }),
                    Token::CoreBasic(CoreBasic { type_path: "core::integer::u128".to_owned() })
                ]
            }))
        );
    }

    #[test]
    fn test_array_type() {
        assert_eq!(
            "Array<[number, number]>",
            JsType::from(&Token::Array(Array {
                type_path: "core::array::Span<(core::integer::u8,core::integer::u128)>".to_owned(),
                inner: Box::new(Token::Tuple(Tuple {
                    type_path: "(core::integer::u8,core::integer::u128)".to_owned(),
                    inners: vec![
                        Token::CoreBasic(CoreBasic { type_path: "core::integer::u8".to_owned() }),
                        Token::CoreBasic(CoreBasic { type_path: "core::integer::u128".to_owned() })
                    ]
                })),
                is_legacy: false,
            }))
        )
    }

    #[test]
    fn test_default_value_basics() {
        assert_eq!(
            "0",
            JsDefaultValue::from(&Token::CoreBasic(CoreBasic {
                type_path: "core::integer::u8".to_owned()
            }))
        );
        assert_eq!(
            "0",
            JsDefaultValue::from(&Token::CoreBasic(CoreBasic {
                type_path: "core::felt252".to_owned()
            }))
        )
    }

    #[test]
    fn test_tuple_default_value() {
        assert_eq!(
            "[0, 0]",
            JsDefaultValue::from(&Token::Tuple(Tuple {
                type_path: "(core::integer::u8,core::integer::u128)".to_owned(),
                inners: vec![
                    Token::CoreBasic(CoreBasic { type_path: "core::integer::u8".to_owned() }),
                    Token::CoreBasic(CoreBasic { type_path: "core::integer::u128".to_owned() })
                ]
            }))
        );
    }

    #[test]
    fn test_array_default_value() {
        assert_eq!(
            "[[0, 0]]",
            JsDefaultValue::from(&Token::Array(Array {
                type_path: "core::array::Span<(core::integer::u8,core::integer::u128)>".to_owned(),
                inner: Box::new(Token::Tuple(Tuple {
                    type_path: "(core::integer::u8,core::integer::u128)".to_owned(),
                    inners: vec![
                        Token::CoreBasic(CoreBasic { type_path: "core::integer::u8".to_owned() }),
                        Token::CoreBasic(CoreBasic { type_path: "core::integer::u128".to_owned() })
                    ]
                })),
                is_legacy: false,
            }))
        )
    }
}
