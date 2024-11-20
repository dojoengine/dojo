use cainome::parser::tokens::{Composite, Token};
use convert_case::{Case, Casing};

pub(crate) mod r#enum;
pub(crate) mod erc;
pub(crate) mod function;
pub(crate) mod interface;
pub(crate) mod schema;

/// Get the namespace and path of a model
/// eg. dojo_examples-actions -> actions
/// or just get the raw type name -> actions
pub(crate) fn get_namespace_and_path(token: &Composite) -> (String, String, String) {
    let ns_split = token.type_path.split("::").collect::<Vec<&str>>();
    if ns_split.len() < 2 {
        panic!("type is invalid type_path has to be at least namespace::type");
    }
    let ns = ns_split[0];
    let type_name = ns_split[ns_split.len() - 1];
    let namespace = ns.to_case(Case::Pascal);
    (ns.to_owned(), namespace, type_name.to_owned())
}

/// Generates default values for each fields of the struct.
pub(crate) fn generate_type_init(token: &Composite) -> String {
    format!(
        "{{\n\t\t\tfieldOrder: [{}],\n{}\n\t\t}}",
        token.inners.iter().map(|i| format!("'{}'", i.name)).collect::<Vec<String>>().join(", "),
        token
            .inners
            .iter()
            .map(|i| {
                match i.token.to_composite() {
                    Ok(c) => {
                        format!("\t\t\t{}: {},", i.name, JsDefaultValue::from(c))
                    }
                    Err(_) => {
                        // this will fail on core types as
                        // `core::starknet::contract_address::ContractAddress`
                        // `core::felt252`
                        // `core::integer::u64`
                        // and so son
                        format!("\t\t\t{}: {},", i.name, JsDefaultValue::from(&i.token))
                    }
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
    )
}

#[derive(Debug)]
pub(crate) struct JsType(String);
impl From<&str> for JsType {
    fn from(value: &str) -> Self {
        match value {
            "felt252" => JsType("BigNumberish".to_owned()),
            "ContractAddress" => JsType("string".to_owned()),
            "ByteArray" => JsType("string".to_owned()),
            "u8" => JsType("BigNumberish".to_owned()),
            "u16" => JsType("BigNumberish".to_owned()),
            "u32" => JsType("BigNumberish".to_owned()),
            "u64" => JsType("BigNumberish".to_owned()),
            "u128" => JsType("BigNumberish".to_owned()),
            "u256" => JsType("BigNumberish".to_owned()),
            "U256" => JsType("BigNumberish".to_owned()),
            "i128" => JsType("BigNumberish".to_owned()),
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
            "i128" => JsDefaultValue("0".to_string()),
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
                "{{ fieldOrder: [{}], {} }}",
                value.inners.iter().map(|i| format!("'{}'", i.name)).collect::<Vec<_>>().join(", "),
                value
                    .inners
                    .iter()
                    .map(|i| format!("{}: {},", i.name, JsDefaultValue::from(&i.token)))
                    .collect::<Vec<_>>()
                    .join(" ")
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
    use cainome::parser::tokens::{
        Array, Composite, CompositeInner, CompositeInnerKind, CompositeType, CoreBasic, Token,
        Tuple,
    };

    use crate::plugins::typescript::generator::{generate_type_init, JsDefaultValue, JsType};

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
            "BigNumberish",
            JsType::from(&Token::CoreBasic(CoreBasic {
                type_path: "core::integer::u8".to_owned()
            }))
        );
        assert_eq!(
            "BigNumberish",
            JsType::from(&Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_owned() }))
        )
    }

    #[test]
    fn test_tuple_type() {
        assert_eq!(
            "[BigNumberish, BigNumberish]",
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
            "Array<[BigNumberish, BigNumberish]>",
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

    #[test]
    fn test_generate_type_init() {
        let token = create_test_struct_token("TestStruct");
        let init_type = generate_type_init(&token);

        // we expect having something like this:
        // the content of generate_type_init is wrapped in a function that adds brackets before and
        // after
        let expected = "{
\t\t\tfieldOrder: ['field1', 'field2', 'field3'],
\t\t\tfield1: 0,
\t\t\tfield2: 0,
\t\t\tfield3: 0,
\t\t}";
        assert_eq!(expected, init_type);
    }
    fn create_test_struct_token(name: &str) -> Composite {
        Composite {
            type_path: format!("onchain_dash::{name}"),
            inners: vec![
                CompositeInner {
                    index: 0,
                    name: "field1".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_owned() }),
                },
                CompositeInner {
                    index: 1,
                    name: "field2".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_owned() }),
                },
                CompositeInner {
                    index: 2,
                    name: "field3".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_owned() }),
                },
            ],
            generic_args: vec![],
            r#type: CompositeType::Struct,
            is_event: false,
            alias: None,
        }
    }
}
