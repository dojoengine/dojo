use cainome::parser::tokens::{Composite, CompositeType};

use crate::{
    error::BindgenResult,
    plugins::{BindgenModelGenerator, Buffer},
};

use super::JsType;

pub(crate) struct TsInterfaceGenerator;

impl BindgenModelGenerator for TsInterfaceGenerator {
    fn generate(&self, token: &Composite, _buffer: &mut Buffer) -> BindgenResult<String> {
        if token.r#type != CompositeType::Struct || token.inners.is_empty() {
            return Ok(String::new());
        }

        Ok(format!(
            "// Type definition for `{path}` struct
export interface {name} {{
\tfieldOrder: string[];
{fields}
}}
",
            path = token.type_path,
            name = token.type_name(),
            fields = token
                .inners
                .iter()
                .map(|inner| { format!("\t{}: {};", inner.name, JsType::from(&inner.token)) })
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

    use crate::plugins::Buffer;

    use super::*;

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

        assert_eq!(result, "// Type definition for `core::test::TestStruct` struct\nexport interface TestStruct {\n\tfieldOrder: string[];\n\tfield1: number;\n\tfield2: number;\n\tfield3: number;\n}\n");
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
}
