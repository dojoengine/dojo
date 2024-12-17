use cainome::parser::tokens::{Composite, CompositeType, Token};
use constants::{
    CAIRO_BOOL, CAIRO_BYTE_ARRAY, CAIRO_CONTRACT_ADDRESS, CAIRO_FELT252, CAIRO_I128, CAIRO_OPTION,
    CAIRO_OPTION_DEFAULT_VALUE, CAIRO_U128, CAIRO_U16, CAIRO_U256, CAIRO_U256_STRUCT, CAIRO_U32,
    CAIRO_U64, CAIRO_U8, JS_BIGNUMBERISH, JS_BOOLEAN, JS_STRING,
};
use convert_case::{Case, Casing};

use crate::plugins::typescript::generator::constants::CAIRO_OPTION_TYPE_PATH;

pub(crate) mod constants;
pub(crate) mod r#enum;
pub(crate) mod function;
pub(crate) mod interface;
pub(crate) mod models;
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
                        format!("\t\t\t{}: {},", i.name, JsPrimitiveDefaultValue::from(c))
                    }
                    Err(_) => {
                        // this will fail on core types as
                        // `core::starknet::contract_address::ContractAddress`
                        // `core::felt252`
                        // `core::integer::u64`
                        // and so son
                        format!("\t\t\t{}: {},", i.name, JsPrimitiveDefaultValue::from(&i.token))
                    }
                }
            })
            .collect::<Vec<String>>()
            .join("\n")
    )
}

/// Checks if Token::Composite is an Option
/// * token - The token to check
pub(crate) fn token_is_option(token: &Composite) -> bool {
    token.type_path.starts_with(CAIRO_OPTION_TYPE_PATH)
}

/// Checks if token has inner composite
/// * token - The token to check
fn token_has_inner_composite(token: &Composite) -> bool {
    token.inners.iter().any(|inner| match &inner.token {
        Token::Array(array) => array.inner.to_composite().is_ok(),
        Token::Tuple(tuple) => tuple.inners.iter().any(|t| matches!(t, Token::Composite(_))),
        Token::Composite(_) => true,
        _ => false,
    })
}

/// Checks if Token::Composite is an custom enum (enum with nested Composite types)
/// * token - The token to check
pub(crate) fn token_is_custom_enum(token: &Composite) -> bool {
    token.r#type == CompositeType::Enum && token_has_inner_composite(token)
}

#[derive(Debug)]
pub(crate) struct JsPrimitiveType(String);
impl From<&str> for JsPrimitiveType {
    fn from(value: &str) -> Self {
        match value {
            CAIRO_FELT252 => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_CONTRACT_ADDRESS => JsPrimitiveType(JS_STRING.to_owned()),
            CAIRO_BYTE_ARRAY => JsPrimitiveType(JS_STRING.to_owned()),
            CAIRO_U8 => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_U16 => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_U32 => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_U64 => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_U128 => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_U256 => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_U256_STRUCT => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_I128 => JsPrimitiveType(JS_BIGNUMBERISH.to_owned()),
            CAIRO_BOOL => JsPrimitiveType(JS_BOOLEAN.to_owned()),
            _ => JsPrimitiveType(value.to_owned()),
        }
    }
}

impl From<&Token> for JsPrimitiveType {
    fn from(value: &Token) -> Self {
        match value {
            Token::Array(a) => JsPrimitiveType::from(
                format!("Array<{}>", JsPrimitiveType::from(&*a.inner)).as_str(),
            ),
            Token::Tuple(t) => JsPrimitiveType::from(
                format!(
                    "[{}]",
                    t.inners
                        .iter()
                        .map(|i| JsPrimitiveType::from(i.type_name().as_str()).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                        .as_str()
                )
                .as_str(),
            ),
            Token::Composite(c) => {
                if token_is_option(c) {
                    return JsPrimitiveType::from(
                        format!(
                            "CairoOption<{}>",
                            c.generic_args
                                .iter()
                                .map(|(_, t)| JsPrimitiveType::from(t).to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                        .as_str(),
                    );
                }
                if token_is_custom_enum(c) {
                    // we defined a type wrapper with Enum suffix let's use it there
                    return JsPrimitiveType::from(format!("{}Enum", c.type_name()).as_str());
                }
                return JsPrimitiveType::from(value.type_name().as_str());
            }
            _ => JsPrimitiveType::from(value.type_name().as_str()),
        }
    }
}

impl std::fmt::Display for JsPrimitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug)]
pub(crate) struct JsPrimitiveDefaultValue(String);
impl From<&str> for JsPrimitiveDefaultValue {
    fn from(value: &str) -> Self {
        match value {
            CAIRO_FELT252 => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_CONTRACT_ADDRESS => JsPrimitiveDefaultValue("\"\"".to_string()),
            CAIRO_BYTE_ARRAY => JsPrimitiveDefaultValue("\"\"".to_string()),
            CAIRO_U8 => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_U16 => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_U32 => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_U64 => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_U128 => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_U256 => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_U256_STRUCT => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_I128 => JsPrimitiveDefaultValue("0".to_string()),
            CAIRO_BOOL => JsPrimitiveDefaultValue("false".to_string()),
            CAIRO_OPTION => JsPrimitiveDefaultValue(CAIRO_OPTION_DEFAULT_VALUE.to_string()),
            _ => JsPrimitiveDefaultValue(value.to_string()),
        }
    }
}
impl From<&Composite> for JsPrimitiveDefaultValue {
    fn from(value: &Composite) -> Self {
        match value.r#type {
            cainome::parser::tokens::CompositeType::Enum => {
                if token_is_custom_enum(value) {
                    return JsPrimitiveDefaultValue(build_custom_enum_default_value(value));
                }
                match value.inners[0].token.to_composite() {
                    Ok(c) => JsPrimitiveDefaultValue::from(c),
                    Err(_) => JsPrimitiveDefaultValue(format!(
                        "{}.{}",
                        value.type_name(),
                        value.inners[0].name
                    )),
                }
            }
            cainome::parser::tokens::CompositeType::Struct => {
                JsPrimitiveDefaultValue(build_struct_default_value(value))
            }
            _ => JsPrimitiveDefaultValue::from(value.type_name().as_str()),
        }
    }
}

/// Builds the default value for an enum variant
/// * token - The enum token to build the default value for
fn build_default_enum_variant(token: &Composite) -> String {
    let default_value = token.inners.iter().take(1).fold(String::new(), |_acc, i| {
        format!(
            "\n\t\t\t\t{}: {}",
            i.name,
            match &i.token {
                Token::CoreBasic(core_basic) =>
                    JsPrimitiveDefaultValue::from(core_basic.type_name().as_str()).0,
                Token::Array(_) => "[]".to_owned(),
                Token::Tuple(_) => "()".to_owned(),
                Token::Composite(composite) => generate_type_init(composite),
                _ => "".to_string(),
            }
        )
    });
    let undefined = token.inners.iter().skip(1).fold(String::new(), |acc, i| {
        format!("{acc}\n\t\t\t\t{}: undefined,", i.name.to_case(Case::Camel))
    });

    default_value + "," + &undefined
}

/// Builds the default value for an enum variant
/// * token - The enum token to build the default value for
fn build_custom_enum_default_value(token: &Composite) -> String {
    format!("new CairoCustomEnum({{ {} }})", build_default_enum_variant(token),)
}

/// Builds the default value for a struct
/// * token - The enum token to build the default value for
fn build_struct_default_value(token: &Composite) -> String {
    format!(
        "{{ fieldOrder: [{}], {} }}",
        token.inners.iter().map(|i| format!("'{}'", i.name)).collect::<Vec<_>>().join(", "),
        token
            .inners
            .iter()
            .map(|i| format!(
                "{}: {},",
                i.name,
                match i.token.to_composite() {
                    Ok(c) => {
                        JsPrimitiveDefaultValue::from(c)
                    }
                    Err(_) => {
                        JsPrimitiveDefaultValue::from(&i.token)
                    }
                }
            ))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

impl From<&Token> for JsPrimitiveDefaultValue {
    fn from(value: &Token) -> Self {
        match value {
            Token::Array(a) => JsPrimitiveDefaultValue::from(
                format!("[{}]", JsPrimitiveDefaultValue::from(&*a.inner)).as_str(),
            ),
            Token::Tuple(t) => JsPrimitiveDefaultValue::from(
                format!(
                    "[{}]",
                    t.inners
                        .iter()
                        .map(|i| JsPrimitiveDefaultValue::from(i.type_name().as_str()).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                        .as_str()
                )
                .as_str(),
            ),
            Token::Composite(c) => JsPrimitiveDefaultValue::from(c),
            _ => JsPrimitiveDefaultValue::from(value.type_name().as_str()),
        }
    }
}

impl std::fmt::Display for JsPrimitiveDefaultValue {
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

    use crate::plugins::typescript::generator::constants::JS_BIGNUMBERISH;
    use crate::plugins::typescript::generator::{
        generate_type_init, JsPrimitiveDefaultValue, JsPrimitiveType,
    };

    impl PartialEq<JsPrimitiveType> for &str {
        fn eq(&self, other: &JsPrimitiveType) -> bool {
            self == &other.0.as_str()
        }
    }

    impl PartialEq<JsPrimitiveDefaultValue> for &str {
        fn eq(&self, other: &JsPrimitiveDefaultValue) -> bool {
            self == &other.0.as_str()
        }
    }

    #[test]
    fn test_js_type_basics() {
        assert_eq!(
            JS_BIGNUMBERISH,
            JsPrimitiveType::from(&Token::CoreBasic(CoreBasic {
                type_path: "core::integer::u8".to_owned()
            }))
        );
        assert_eq!(
            JS_BIGNUMBERISH,
            JsPrimitiveType::from(&Token::CoreBasic(CoreBasic {
                type_path: "core::felt252".to_owned()
            }))
        )
    }

    #[test]
    fn test_tuple_type() {
        assert_eq!(
            format!("[{}, {}]", JS_BIGNUMBERISH, JS_BIGNUMBERISH).as_str(),
            JsPrimitiveType::from(&Token::Tuple(Tuple {
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
            format!("Array<[{}, {}]>", JS_BIGNUMBERISH, JS_BIGNUMBERISH).as_str(),
            JsPrimitiveType::from(&Token::Array(Array {
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
    fn test_option_type() {
        assert_eq!(
            "CairoOption<GatedType>",
            JsPrimitiveType::from(&Token::Composite(Composite {
                type_path: "core::option::Option<tournament::ls15_components::models::tournament::GatedType>".to_owned(),
                inners: vec![],
                generic_args: vec![
                        (
                        "A".to_owned(), 
                        Token::Composite(
                            Composite {
                                type_path: "tournament::ls15_components::models::tournament::GatedType".to_owned(), 
                                inners: vec![
                                    CompositeInner {
                                        index: 0,
                                        name: "token".to_owned(),
                                        kind: CompositeInnerKind::NotUsed,
                                        token: Token::Composite(
                                            Composite {
                                                type_path: "tournament::ls15_components::models::tournament::GatedToken".to_owned(),
                                                inners: vec![],
                                                generic_args: vec![],
                                                r#type: CompositeType::Unknown,
                                                is_event: false,
                                                alias: None,
                                            },
                                        ),
                                    },
                                    CompositeInner {
                                        index: 1,
                                        name: "tournament".to_owned(),
                                        kind: CompositeInnerKind::NotUsed,
                                        token: Token::Array(
                                            Array {
                                                type_path: "core::array::Span::<core::integer::u64>".to_owned(),
                                                inner: Box::new(Token::CoreBasic(
                                                    CoreBasic {
                                                        type_path: "core::integer::u64".to_owned(),
                                                    },
                                               )),
                                                is_legacy: false,
                                            },
                                        ),
                                    },
                                    CompositeInner {
                                        index: 2,
                                        name: "address".to_owned(),
                                        kind: CompositeInnerKind::NotUsed,
                                        token: Token::Array(
                                            Array {
                                                type_path: "core::array::Span::<core::starknet::contract_address::ContractAddress>".to_owned(),
                                                inner: Box::new(Token::CoreBasic(
                                                    CoreBasic {
                                                        type_path: "core::starknet::contract_address::ContractAddress".to_owned(),
                                                    },
                                                )) ,
                                                is_legacy: false,
                                            },
                                        ),
                                    }
                                ],
                                generic_args: vec![],
                                r#type: CompositeType::Unknown,
                                is_event: false,
                                alias: None
                            }
                        )
                    ),
                ],
                r#type: CompositeType::Unknown,
                is_event: false,
                alias: None            }))
        )
    }

    #[test]
    fn test_default_value_basics() {
        assert_eq!(
            "0",
            JsPrimitiveDefaultValue::from(&Token::CoreBasic(CoreBasic {
                type_path: "core::integer::u8".to_owned()
            }))
        );
        assert_eq!(
            "0",
            JsPrimitiveDefaultValue::from(&Token::CoreBasic(CoreBasic {
                type_path: "core::felt252".to_owned()
            }))
        )
    }

    #[test]
    fn test_tuple_default_value() {
        assert_eq!(
            "[0, 0]",
            JsPrimitiveDefaultValue::from(&Token::Tuple(Tuple {
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
            JsPrimitiveDefaultValue::from(&Token::Array(Array {
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
    fn test_enum_default_value() {
        assert_eq!(
            "Direction.Up",
            JsPrimitiveDefaultValue::from(&Token::Composite(Composite {
                type_path: "dojo_starter::Direction".to_owned(),
                inners: vec![
                    CompositeInner {
                        index: 0,
                        name: "Up".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic { type_path: "()".to_owned() })
                    },
                    CompositeInner {
                        index: 1,
                        name: "Down".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic { type_path: "()".to_owned() })
                    },
                    CompositeInner {
                        index: 2,
                        name: "Left".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic { type_path: "()".to_owned() })
                    },
                    CompositeInner {
                        index: 3,
                        name: "Right".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic { type_path: "()".to_owned() })
                    },
                ],
                generic_args: vec![],
                r#type: CompositeType::Enum,
                is_event: false,
                alias: None,
            }))
        )
    }

    #[test]
    fn test_cairo_custom_enum_default_value() {
        assert_eq!(
            "new CairoCustomEnum({ \n\t\t\t\titem: {\n\t\t\tfieldOrder: ['id', 'xp'],\n\t\t\tid: \
             0,\n\t\t\txp: 0,\n\t\t},\n\t\t\t\taddress: undefined, })",
            JsPrimitiveDefaultValue::from(&Token::Composite(Composite {
                type_path: "dojo_starter::Direction".to_owned(),
                inners: vec![
                    CompositeInner {
                        index: 0,
                        name: "item".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::Composite(Composite {
                            type_path: "dojo_starter::Item".to_owned(),
                            inners: vec![
                                CompositeInner {
                                    index: 0,
                                    name: "id".to_owned(),
                                    kind: CompositeInnerKind::NotUsed,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::felt252".to_owned(),
                                    })
                                },
                                CompositeInner {
                                    index: 1,
                                    name: "xp".to_owned(),
                                    kind: CompositeInnerKind::NotUsed,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::felt252".to_owned(),
                                    })
                                },
                            ],
                            generic_args: vec![],
                            r#type: CompositeType::Struct,
                            is_event: false,
                            alias: None,
                        })
                    },
                    CompositeInner {
                        index: 1,
                        name: "address".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic { type_path: "()".to_owned() })
                    },
                ],
                generic_args: vec![],
                r#type: CompositeType::Enum,
                is_event: false,
                alias: None,
            }))
        )
    }

    #[test]
    fn test_composite_default_value() {
        assert_eq!(
            "{ fieldOrder: ['id', 'xp'], id: 0, xp: 0, }",
            JsPrimitiveDefaultValue::from(&Token::Composite(Composite {
                type_path: "dojo_starter::Item".to_owned(),
                inners: vec![
                    CompositeInner {
                        index: 0,
                        name: "id".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic {
                            type_path: "core::felt252".to_owned(),
                        })
                    },
                    CompositeInner {
                        index: 1,
                        name: "xp".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic {
                            type_path: "core::felt252".to_owned(),
                        })
                    },
                ],
                generic_args: vec![],
                r#type: CompositeType::Struct,
                is_event: false,
                alias: None,
            }))
        )
    }

    #[test]
    fn test_nested_composite_default_value() {
        assert_eq!(
            "{ fieldOrder: ['id', 'xp', 'item'], id: 0, xp: 0, item: { fieldOrder: ['id', 'xp', \
             'item'], id: 0, xp: 0, item: { fieldOrder: ['id', 'xp'], id: 0, xp: 0, }, }, }",
            JsPrimitiveDefaultValue::from(&Token::Composite(Composite {
                type_path: "dojo_starter::Item".to_owned(),
                inners: vec![
                    CompositeInner {
                        index: 0,
                        name: "id".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic {
                            type_path: "core::felt252".to_owned(),
                        })
                    },
                    CompositeInner {
                        index: 1,
                        name: "xp".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic {
                            type_path: "core::felt252".to_owned(),
                        })
                    },
                    CompositeInner {
                        index: 1,
                        name: "item".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::Composite(Composite {
                            type_path: "dojo_starter::Item".to_owned(),
                            inners: vec![
                                CompositeInner {
                                    index: 0,
                                    name: "id".to_owned(),
                                    kind: CompositeInnerKind::NotUsed,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::felt252".to_owned(),
                                    })
                                },
                                CompositeInner {
                                    index: 1,
                                    name: "xp".to_owned(),
                                    kind: CompositeInnerKind::NotUsed,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::felt252".to_owned(),
                                    })
                                },
                                CompositeInner {
                                    index: 1,
                                    name: "item".to_owned(),
                                    kind: CompositeInnerKind::NotUsed,
                                    token: Token::Composite(Composite {
                                        type_path: "dojo_starter::Item".to_owned(),
                                        inners: vec![
                                            CompositeInner {
                                                index: 0,
                                                name: "id".to_owned(),
                                                kind: CompositeInnerKind::NotUsed,
                                                token: Token::CoreBasic(CoreBasic {
                                                    type_path: "core::felt252".to_owned(),
                                                })
                                            },
                                            CompositeInner {
                                                index: 1,
                                                name: "xp".to_owned(),
                                                kind: CompositeInnerKind::NotUsed,
                                                token: Token::CoreBasic(CoreBasic {
                                                    type_path: "core::felt252".to_owned(),
                                                })
                                            },
                                        ],
                                        generic_args: vec![],
                                        r#type: CompositeType::Struct,
                                        is_event: false,
                                        alias: None,
                                    })
                                },
                            ],
                            generic_args: vec![],
                            r#type: CompositeType::Struct,
                            is_event: false,
                            alias: None,
                        })
                    },
                ],
                generic_args: vec![],
                r#type: CompositeType::Struct,
                is_event: false,
                alias: None,
            }))
        )
    }

    #[test]
    fn test_cairo_option_default_value() {
        assert_eq!(
            "new CairoOption(CairoOptionVariant.None)",
            JsPrimitiveDefaultValue::from(&Token::Composite(Composite {
                type_path: "core::option::Option<dojo_starter::Item>".to_owned(),
                inners: vec![
                    CompositeInner {
                        index: 0,
                        name: "id".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic {
                            type_path: "core::felt252".to_owned(),
                        })
                    },
                    CompositeInner {
                        index: 1,
                        name: "xp".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::CoreBasic(CoreBasic {
                            type_path: "core::felt252".to_owned(),
                        })
                    },
                ],
                generic_args: vec![],
                r#type: CompositeType::Unknown,
                is_event: false,
                alias: None,
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

    #[test]
    fn test_option_custom_enum_type() {
        let token = create_option_custom_enum_token();
        assert_eq!("{ fieldOrder: [],  }", JsPrimitiveDefaultValue::from(&Token::Composite(token)))
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

    fn create_option_custom_enum_token() -> Composite {
        Composite {
            type_path: "core::option::Option<core::test::CustomEnum>".to_owned(),
            inners: vec![],
            generic_args: vec![(
                "A".to_owned(),
                Token::Composite(Composite {
                    type_path: "core::test::CustomEnum".to_owned(),
                    inners: vec![
                        CompositeInner {
                            index: 0,
                            name: "Variant1".to_owned(),
                            kind: CompositeInnerKind::NotUsed,
                            token: Token::CoreBasic(CoreBasic {
                                type_path: "core::felt252".to_owned(),
                            }),
                        },
                        CompositeInner {
                            index: 1,
                            name: "Variant2".to_owned(),
                            kind: CompositeInnerKind::NotUsed,
                            token: Token::Composite(Composite {
                                type_path: "core::test::NestedStruct".to_owned(),
                                inners: vec![CompositeInner {
                                    index: 0,
                                    name: "nested_field".to_owned(),
                                    kind: CompositeInnerKind::Key,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::felt252".to_owned(),
                                    }),
                                }],
                                generic_args: vec![],
                                r#type: CompositeType::Struct,
                                is_event: false,
                                alias: None,
                            }),
                        },
                    ],
                    generic_args: vec![],
                    r#type: CompositeType::Enum,
                    is_event: false,
                    alias: None,
                }),
            )],
            r#type: CompositeType::Struct,
            is_event: false,
            alias: None,
        }
    }
}
