use cainome::parser::tokens::{Composite, CompositeType};
use convert_case::{Case, Casing};

use super::constants::{CAIRO_ENUM_IMPORT, CAIRO_ENUM_TOKEN, SN_IMPORT_SEARCH};
use super::{token_is_custom_enum, token_is_enum};
use crate::error::BindgenResult;
use crate::plugins::typescript::generator::JsPrimitiveType;
use crate::plugins::{BindgenModelGenerator, Buffer};

pub(crate) struct TsEnumGenerator;

impl TsEnumGenerator {
    fn check_import(&self, token: &Composite, buffer: &mut Buffer) {
        // type is Enum with type variants, need to import CairoEnum
        // if enum has at least one inner that is a composite type
        if token_is_enum(token) {
            if !buffer.has(SN_IMPORT_SEARCH) {
                buffer.push(CAIRO_ENUM_IMPORT.to_owned());
            } else if !buffer.has(CAIRO_ENUM_TOKEN) {
                // If 'starknet' import is present, we add CairoEnum to the imported types
                buffer.insert_after(format!(" {CAIRO_ENUM_TOKEN}"), SN_IMPORT_SEARCH, "{", 1);
            }
        }
    }

    fn generate_simple_enum(&self, token: &Composite) -> BindgenResult<String> {
        Ok(format!(
            "// Type definition for `{path}` enum
export const {camel_name} = [
{variants}
] as const;
export type {name} = {{ [key in typeof {camel_name}[number]]: string }};
export type {name}Enum = CairoCustomEnum;
",
            path = token.type_path,
            name = token.type_name(),
            camel_name = token.type_name().to_case(Case::Camel),
            variants = token
                .inners
                .iter()
                .map(|inner| { format!("\t'{}',", inner.name) })
                .collect::<Vec<String>>()
                .join("\n")
        ))
    }

    fn generate_custom_enum(&self, token: &Composite) -> BindgenResult<String> {
        Ok(format!(
            "// Type definition for `{path}` enum
export const {camel_name} = [
{variants}
] as const;
export type {name} = {{ 
{variant_types}
}};
export type {name}Enum = CairoCustomEnum;
",
            path = token.type_path,
            name = token.type_name(),
            camel_name = token.type_name().to_case(Case::Camel),
            variants = token
                .inners
                .iter()
                .map(|inner| { format!("\t'{}',", inner.name) })
                .collect::<Vec<String>>()
                .join("\n"),
            variant_types = token
                .inners
                .iter()
                .map(|inner| {
                    format!("\t{}: {},", inner.name, JsPrimitiveType::from(&inner.token))
                })
                .collect::<Vec<String>>()
                .join("\n")
        ))
    }
}

impl BindgenModelGenerator for TsEnumGenerator {
    fn generate(&self, token: &Composite, buffer: &mut Buffer) -> BindgenResult<String> {
        if token.r#type != CompositeType::Enum || token.inners.is_empty() {
            return Ok(String::new());
        }

        self.check_import(token, buffer);
        let gen = if token_is_custom_enum(token) {
            self.generate_custom_enum(token)?
        } else {
            self.generate_simple_enum(token)?
        };

        if buffer.has(&gen) {
            return Ok(String::new());
        }

        Ok(gen)
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
    fn test_enumeration_without_inners() {
        let mut buff = Buffer::new();
        let writer = TsEnumGenerator;
        let token = Composite {
            type_path: "core::test::Test".to_owned(),
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
    fn test_enumeration_not_enum() {
        let mut buff = Buffer::new();
        let writer = TsEnumGenerator;
        let token = Composite {
            type_path: "core::test::Test".to_owned(),
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
    fn test_enumeration_with_inners() {
        let mut buff = Buffer::new();
        let writer = TsEnumGenerator;
        let token = create_available_theme_enum_token();
        let result = writer.generate(&token, &mut buff).unwrap();

        assert_eq!(
            result,
            "// Type definition for `core::test::AvailableTheme` enum\nexport const \
             availableTheme = [\n\t'Light',\n\t'Dark',\n\t'Dojo',\n] as const;\nexport type \
             AvailableTheme = { [key in typeof availableTheme[number]]: string };\nexport type \
             AvailableThemeEnum = CairoCustomEnum;\n"
        );
    }

    #[test]
    fn test_it_does_not_duplicates_enum() {
        let mut buff = Buffer::new();
        let writer = TsEnumGenerator;
        buff.push(
            "// Type definition for `core::test::AvailableTheme` enum\nexport const \
             availableTheme = [\n\t'Light',\n\t'Dark',\n\t'Dojo',\n] as const;\nexport type \
             AvailableTheme = { [key in typeof availableTheme[number]]: string };\nexport type \
             AvailableThemeEnum = CairoCustomEnum;\n"
                .to_owned(),
        );

        let token_dup = create_available_theme_enum_token();
        let result = writer.generate(&token_dup, &mut buff).unwrap();
        // Length is 2 because we add import of CairoCustomEnum
        assert_eq!(buff.len(), 2);
        assert!(result.is_empty())
    }

    #[test]
    fn test_custom_enum() {
        let mut buff = Buffer::new();
        let writer = TsEnumGenerator;
        let token = create_custom_enum_token();
        let result = writer.generate(&token, &mut buff).unwrap();
        assert_eq!(
            result,
            "// Type definition for `core::test::CustomEnum` enum\nexport const customEnum = \
             [\n\t'Predefined',\n\t'Custom',\n] as const;\nexport type CustomEnum = { \
             \n\tPredefined: AvailableThemeEnum,\n\tCustom: Custom,\n};\nexport type \
             CustomEnumEnum = CairoCustomEnum;\n"
        );
    }

    fn create_available_theme_enum_token() -> Composite {
        Composite {
            type_path: "core::test::AvailableTheme".to_owned(),
            inners: vec![
                CompositeInner {
                    index: 0,
                    name: "Light".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::CoreBasic(CoreBasic { type_path: "()".to_owned() }),
                },
                CompositeInner {
                    index: 1,
                    name: "Dark".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::CoreBasic(CoreBasic { type_path: "()".to_owned() }),
                },
                CompositeInner {
                    index: 2,
                    name: "Dojo".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::CoreBasic(CoreBasic { type_path: "()".to_owned() }),
                },
            ],
            generic_args: vec![],
            r#type: CompositeType::Enum,
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
                    name: "Predefined".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::Composite(create_available_theme_enum_token()),
                },
                CompositeInner {
                    index: 1,
                    name: "Custom".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::Composite(Composite {
                        type_path: "core::test::custom::Custom".to_owned(),
                        inners: vec![CompositeInner {
                            index: 0,
                            name: "Classname".to_owned(),
                            kind: CompositeInnerKind::Key,
                            token: Token::CoreBasic(CoreBasic { type_path: "felt252".to_owned() }),
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
