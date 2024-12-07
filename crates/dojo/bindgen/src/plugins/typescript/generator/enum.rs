use cainome::parser::tokens::{Composite, CompositeType};

use super::constants::{CAIRO_ENUM_IMPORT, CAIRO_ENUM_TOKEN, SN_IMPORT_SEARCH};
use super::token_is_custom_enum;
use crate::error::BindgenResult;
use crate::plugins::typescript::generator::JsType;
use crate::plugins::{BindgenModelGenerator, Buffer};

const CAIRO_ENUM_TYPE_IMPL: &str = "export type TypedCairoEnum<T> = CairoCustomEnum & \
                                    {\n\tvariant: { [K in keyof T]: T[K] | undefined \
                                    };\n\tunwrap(): T[keyof T];\n}\n";

pub(crate) struct TsEnumGenerator;

impl TsEnumGenerator {
    fn check_import(&self, token: &Composite, buffer: &mut Buffer) {
        // type is Enum with type variants, need to import CairoEnum
        // if enum has at least one inner that is a composite type
        if token_is_custom_enum(token) {
            if !buffer.has(SN_IMPORT_SEARCH) {
                buffer.push(CAIRO_ENUM_IMPORT.to_owned());
            } else if !buffer.has(CAIRO_ENUM_TOKEN) {
                // If 'starknet' import is present, we add CairoEnum to the imported types
                buffer.insert_after(format!(" {CAIRO_ENUM_TOKEN}"), SN_IMPORT_SEARCH, "{", 1);
            }
        }
        if !buffer.has(CAIRO_ENUM_TYPE_IMPL) {
            let pos = buffer.pos(SN_IMPORT_SEARCH).unwrap();
            buffer.insert_at_index(CAIRO_ENUM_TYPE_IMPL.to_owned(), pos + 1);
        }
    }
}

impl BindgenModelGenerator for TsEnumGenerator {
    fn generate(&self, token: &Composite, buffer: &mut Buffer) -> BindgenResult<String> {
        if token.r#type != CompositeType::Enum || token.inners.is_empty() {
            return Ok(String::new());
        }

        let gen = if token_is_custom_enum(token) {
            self.check_import(token, buffer);
            format!(
                "// Type definition for `{path}` enum
export type {name} = {{
{variants}
}}
export type {name}Enum = TypedCairoEnum<{name}>;
",
                path = token.type_path,
                name = token.type_name(),
                variants = token
                    .inners
                    .iter()
                    .map(|inner| { format!("\t{}: {};", inner.name, JsType::from(&inner.token)) })
                    .collect::<Vec<String>>()
                    .join("\n")
            )
        } else {
            format!(
                "// Type definition for `{path}` enum
export enum {name} {{
{variants}
}}
",
                path = token.type_path,
                name = token.type_name(),
                variants = token
                    .inners
                    .iter()
                    .map(|inner| format!("\t{},", inner.name))
                    .collect::<Vec<String>>()
                    .join("\n")
            )
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
            "// Type definition for `core::test::AvailableTheme` enum\nexport enum AvailableTheme \
             {\n\tLight,\n\tDark,\n\tDojo,\n}\n"
        );
    }

    #[test]
    fn test_it_does_not_duplicates_enum() {
        let mut buff = Buffer::new();
        let writer = TsEnumGenerator;
        buff.push(
            "// Type definition for `core::test::AvailableTheme` enum\nexport enum AvailableTheme \
             {\n\tLight,\n\tDark,\n\tDojo,\n}\n"
                .to_owned(),
        );

        let token_dup = create_available_theme_enum_token();
        let result = writer.generate(&token_dup, &mut buff).unwrap();
        assert_eq!(buff.len(), 1);
        assert!(result.is_empty())
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
}
