use cainome::parser::tokens::{Composite, CompositeType};

use crate::{error::BindgenResult, plugins::BindgenModelGenerator};

pub(crate) struct TsEnumGenerator;

impl BindgenModelGenerator for TsEnumGenerator {
    fn generate(&self, token: &Composite, buffer: &mut Vec<String>) -> BindgenResult<String> {
        if token.r#type != CompositeType::Enum || token.inners.is_empty() {
            return Ok(String::new());
        }

        let gen = format!(
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
        );

        if buffer.iter().any(|b| b.contains(&gen)) {
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

    #[test]
    fn test_enumeration_without_inners() {
        let mut buff: Vec<String> = Vec::new();
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
        let mut buff: Vec<String> = Vec::new();
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
        let mut buff: Vec<String> = Vec::new();
        let writer = TsEnumGenerator;
        let token = create_available_theme_enum_token();
        let result = writer.generate(&token, &mut buff).unwrap();

        assert_eq!(result, "// Type definition for `core::test::AvailableTheme` enum\nexport enum AvailableTheme {\n\tLight,\n\tDark,\n\tDojo,\n}\n");
    }

    #[test]
    fn test_it_does_not_duplicates_enum() {
        let mut buff: Vec<String> = Vec::new();
        let writer = TsEnumGenerator;
        buff.push("// Type definition for `core::test::AvailableTheme` enum\nexport enum AvailableTheme {\n\tLight,\n\tDark,\n\tDojo,\n}\n".to_owned());

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
