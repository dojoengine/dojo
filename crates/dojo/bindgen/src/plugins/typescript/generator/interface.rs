use cainome::parser::tokens::{Composite, CompositeType, Token};

use super::constants::{BIGNUMBERISH_IMPORT, CAIRO_OPTION_IMPORT, SN_IMPORT_SEARCH};
use super::{token_is_option, JsPrimitiveType};
use crate::error::BindgenResult;
use crate::plugins::typescript::generator::constants::CAIRO_OPTION_TOKEN;
use crate::plugins::{BindgenModelGenerator, Buffer};

pub(crate) struct TsInterfaceGenerator;
impl TsInterfaceGenerator {
    fn check_import(&self, token: &Composite, buffer: &mut Buffer) {
        // only search for end part of the import, as we append the other imports afterward
        if !buffer.has("BigNumberish } from 'starknet';") {
            buffer.push(BIGNUMBERISH_IMPORT.to_owned());
        }

        // type is Option, need to import CairoOption
        if token_is_option(token) {
            // we directly add import if 'starknet' import is not present
            if !buffer.has(SN_IMPORT_SEARCH) {
                buffer.push(CAIRO_OPTION_IMPORT.to_owned());
            } else if !buffer.has(CAIRO_OPTION_TOKEN) {
                // If 'starknet' import is present, we add CairoOption to the imported types
                buffer.insert_after(format!(" {CAIRO_OPTION_TOKEN}"), SN_IMPORT_SEARCH, "{", 1);
            }
        }
    }
}

impl BindgenModelGenerator for TsInterfaceGenerator {
    fn generate(&self, token: &Composite, buffer: &mut Buffer) -> BindgenResult<String> {
        if token.r#type != CompositeType::Struct || token.inners.is_empty() {
            return Ok(String::new());
        }
        if buffer
            .has(format!("// Type definition for `{path}` struct", path = token.type_path).as_str())
        {
            return Ok(String::new());
        }

        self.check_import(token, buffer);

        Ok(format!(
            "// Type definition for `{path}` struct
export interface {name} {{
{fields}
}}
",
            path = token.type_path,
            name = token.type_name(),
            fields = token
                .inners
                .iter()
                .map(|inner| {
                    if let Token::Composite(composite) = &inner.token {
                        if token_is_option(composite) {
                            self.check_import(composite, buffer);
                        }
                    }

                    format!("\t{}: {};", inner.name, JsPrimitiveType::from(&inner.token))
                })
                .collect::<Vec<String>>()
                .join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use cainome::parser::tokens::{
        CompositeInner, CompositeInnerKind, CompositeType, CoreBasic, Token,
    };

    use super::*;
    use crate::plugins::Buffer;

    #[test]
    fn test_interface_without_inners() {
        let mut buff = Buffer::new();
        let writer = TsInterfaceGenerator;
        let token = Composite {
            type_path: "core::test::Test".to_string(),
            inners: vec![],
            generic_args: vec![],
            r#type: CompositeType::Struct,
            is_event: false,
            alias: None,
        };
        let result = writer.generate(&token, &mut buff).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_interface_not_struct() {
        let mut buff = Buffer::new();
        let writer = TsInterfaceGenerator;
        let token = Composite {
            type_path: "core::test::Test".to_string(),
            inners: vec![],
            generic_args: vec![],
            r#type: CompositeType::Enum,
            is_event: false,
            alias: None,
        };
        let result = writer.generate(&token, &mut buff).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_interface_with_inners() {
        let mut buff = Buffer::new();
        let writer = TsInterfaceGenerator;
        let token = create_test_struct_token();
        let result = writer.generate(&token, &mut buff).unwrap();

        assert_eq!(
            result,
            "// Type definition for `core::test::TestStruct` struct\nexport interface TestStruct \
             {\n\tfield1: BigNumberish;\n\tfield2: BigNumberish;\n\tfield3: BigNumberish;\n}\n"
        );
    }

    #[test]
    fn test_check_import() {
        let mut buff = Buffer::new();
        let writer = TsInterfaceGenerator;
        let token = create_test_struct_token();
        writer.check_import(&token, &mut buff);
        assert_eq!(1, buff.len());
        let option = create_option_token();
        writer.check_import(&option, &mut buff);
        assert_eq!(1, buff.len());
        let custom_enum = create_custom_enum_token();
        writer.check_import(&custom_enum, &mut buff);
        assert_eq!(1, buff.len());
    }

    fn create_test_struct_token() -> Composite {
        Composite {
            type_path: "core::test::TestStruct".to_owned(),
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

    fn create_option_token() -> Composite {
        Composite {
            type_path: "core::option::Option<core::felt252>".to_owned(),
            inners: vec![CompositeInner {
                index: 0,
                name: "value".to_owned(),
                kind: CompositeInnerKind::Key,
                token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_owned() }),
            }],
            generic_args: vec![],
            r#type: CompositeType::Struct,
            is_event: false,
            alias: None,
        }
    }
    fn create_custom_enum_token() -> Composite {
        Composite {
            type_path: "core::test::CustomEnum".to_owned(),
            inners: vec![
                CompositeInner {
                    index: 0,
                    name: "Variant1".to_owned(),
                    kind: CompositeInnerKind::NotUsed,
                    token: Token::CoreBasic(CoreBasic { type_path: "core::felt252".to_owned() }),
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
        }
    }
}
