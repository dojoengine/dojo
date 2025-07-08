use cainome::parser::tokens::{Composite, CompositeType};

use super::{generate_type_init, get_namespace_and_path};
use crate::error::BindgenResult;
use crate::plugins::{BindgenModelGenerator, Buffer};

/// This generator will build a schema based on previous generator results.
/// first we need to generate interface of schema which will be used in dojo.js sdk to fully type
/// sdk
/// then we need to build the schema const which contains default values for all fieleds
pub(crate) struct TsSchemaGenerator {}
impl TsSchemaGenerator {
    /// Import only needs to be present once
    fn import_schema_type(&self, buffer: &mut Buffer) {
        if !buffer.has("import type { SchemaType as ISchemaType }") {
            buffer.insert(
                0,
                "import type { SchemaType as ISchemaType } from \"@dojoengine/sdk\";\n".to_owned(),
            );
        }
    }

    /// Generates the type definition for the schema
    fn handle_schema_type(&self, token: &Composite, buffer: &mut Buffer) {
        let (_original_ns, _namespace, type_name) = get_namespace_and_path(token);

        // Extract world name from buffer if available
        let ns = self.get_world_name_from_buffer(buffer).unwrap_or(_original_ns);

        let schema_type = "export interface SchemaType extends ISchemaType";
        if !buffer.has(schema_type) {
            buffer.push(format!(
                "export interface SchemaType extends ISchemaType {{\n\t{ns}: {{\n\t\t{type_name}: \
                 {type_name},\n\t}},\n}}",
            ));
            return;
        }

        // check if namespace is defined in interface. if not, add it.
        // next, find where namespace was defined in interface and add property to it.
        if !self.namespace_is_defined(buffer, &ns) {
            let gen = format!("\n\t{ns}: {{\n\t\t{type_name}: {type_name},\n\t}},");
            buffer.insert_after(gen, schema_type, ",", 1);
            return;
        }

        // type has already been initialized
        let gen = format!("\n\t\t{type_name}: {type_name},");
        if buffer.has(&gen) {
            return;
        }

        let ns_def = format!("\n\t{ns}: {{\n\t\t");

        // fastest way to add a field to the interface is to search for the n-1
        // `,` and add the
        // field directly after it.
        // to improve this logic, we would need to either have some kind of code parsing.
        // we could otherwise have some intermediate representation that we pass to this generator
        // function.
        buffer.insert_after(gen, &ns_def, ",", 2);
    }

    /// Generates the default values for the schema
    fn handle_schema_const(&self, token: &Composite, buffer: &mut Buffer) {
        let (_original_ns, _namespace, type_name) = get_namespace_and_path(token);

        // Extract world name from buffer if available
        let ns = self.get_world_name_from_buffer(buffer).unwrap_or(_original_ns);

        let const_type = "export const schema: SchemaType";
        if !buffer.has(const_type) {
            buffer.push(format!(
                "export const schema: SchemaType = {{\n\t{ns}: {{\n\t\t{}: {},\n\t}},\n}};",
                type_name,
                generate_type_init(token)
            ));
            return;
        }

        // check if namespace is defined in interface. if not, add it.
        // next, find where namespace was defined in interface and add property to it.
        if !self.namespace_is_defined(buffer, &ns) {
            let gen =
                format!("\n\t{ns}: {{\n\t\t{}: {},\n\t}},", type_name, generate_type_init(token));
            buffer.insert_after(gen, const_type, ",", 1);
            return;
        }

        // type has already been initialized
        let gen = format!("\n\t\t{type_name}: {},", generate_type_init(token));
        if buffer.has(&gen) {
            return;
        }

        buffer.insert_after(gen, const_type, ",", 2);
    }

    /// Check if namespace is defined in schema
    fn namespace_is_defined(&self, buffer: &mut Buffer, ns: &str) -> bool {
        buffer.has(format!("\n\t{ns}: {{\n\t\t").as_str())
    }

    /// Extract world name from buffer
    fn get_world_name_from_buffer(&self, buffer: &Buffer) -> Option<String> {
        // Look for the special marker we added in the writer
        for line in buffer.iter() {
            if line.starts_with("// WORLD_NAME:") {
                return Some(line.trim_start_matches("// WORLD_NAME:").to_string());
            }
        }
        None
    }
}

impl BindgenModelGenerator for TsSchemaGenerator {
    fn generate(&self, token: &Composite, buffer: &mut Buffer) -> BindgenResult<String> {
        if token.inners.is_empty() || token.r#type != CompositeType::Struct {
            return Ok(String::new());
        }
        self.import_schema_type(buffer);

        // in buffer search for interface named SchemaType extends
        // ISchemaType
        // this should be hold in a buffer item
        self.handle_schema_type(token, buffer);

        // in buffer search for const schema: SchemaType =  named
        // {pascal_case(namespace)}SchemaType extends SchemaType
        // this should be hold in a buffer item
        self.handle_schema_const(token, buffer);

        Ok(String::new())
    }
}

/// Those tests may not test the returned value because it is supporsed to be called sequentially
/// after other generators have been called.
/// This generator will be state based on an external mutable buffer which is a carry
#[cfg(test)]
mod tests {
    use cainome::parser::tokens::{
        CompositeInner, CompositeInnerKind, CompositeType, CoreBasic, Token,
    };

    use super::*;
    use crate::plugins::BindgenModelGenerator;

    #[test]
    fn test_it_does_nothing_if_no_inners() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Buffer::new();

        let token = Composite {
            type_path: "core::test::Test".to_owned(),
            inners: vec![],
            generic_args: vec![],
            r#type: CompositeType::Enum,
            is_event: false,
            alias: None,
        };
        let _result = generator.generate(&token, &mut buffer);
        assert_eq!(0, buffer.len());
    }

    #[test]
    fn test_it_adds_imports() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Buffer::new();

        let token = create_test_struct_token("TestStruct", "onchain_dash");
        let _result = generator.generate(&token, &mut buffer);

        // token is not empty, we should have an import
        assert_eq!(
            "import type { SchemaType as ISchemaType } from \"@dojoengine/sdk\";\n",
            buffer[0]
        );
    }

    /// NOTE: For the following tests, we assume that the `enum.rs` and `interface.rs` generators
    /// successfully ran and generated related output to generator base interfaces + enums.
    #[test]
    fn test_it_appends_schema_type() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Buffer::new();

        let token = create_test_struct_token("TestStruct", "onchain_dash");
        let _result = generator.generate(&token, &mut buffer);
        assert_eq!(
            "export interface SchemaType extends ISchemaType {\n\tonchain_dash: \
             {\n\t\tTestStruct: TestStruct,\n\t},\n}",
            buffer[1]
        );
    }

    #[test]
    fn test_handle_schema_type() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Buffer::new();

        let token = create_test_struct_token("TestStruct", "onchain_dash");
        generator.handle_schema_type(&token, &mut buffer);

        assert_ne!(0, buffer.len());
        assert_eq!(
            "export interface SchemaType extends ISchemaType {\n\tonchain_dash: \
             {\n\t\tTestStruct: TestStruct,\n\t},\n}",
            buffer[0]
        );

        let token_2 = create_test_struct_token("AvailableTheme", "onchain_dash");
        generator.handle_schema_type(&token_2, &mut buffer);
        assert_eq!(
            "export interface SchemaType extends ISchemaType {\n\tonchain_dash: \
             {\n\t\tTestStruct: TestStruct,\n\t\tAvailableTheme: AvailableTheme,\n\t},\n}",
            buffer[0]
        );
        let token_3 = create_test_struct_token("Player", "combat");
        generator.handle_schema_type(&token_3, &mut buffer);
        assert_eq!(
            "export interface SchemaType extends ISchemaType {\n\tonchain_dash: \
             {\n\t\tTestStruct: TestStruct,\n\t\tAvailableTheme: AvailableTheme,\n\t},\n\tcombat: \
             {\n\t\tPlayer: Player,\n\t},\n}",
            buffer[0]
        );
        let token_4 = create_test_struct_token("Position", "combat");
        generator.handle_schema_type(&token_4, &mut buffer);
        assert_eq!(
            "export interface SchemaType extends ISchemaType {\n\tonchain_dash: \
             {\n\t\tTestStruct: TestStruct,\n\t\tAvailableTheme: AvailableTheme,\n\t},\n\tcombat: \
             {\n\t\tPlayer: Player,\n\t\tPosition: Position,\n\t},\n}",
            buffer[0]
        );
    }

    #[test]
    fn test_handle_schema_const() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Buffer::new();
        let token = create_test_struct_token("TestStruct", "onchain_dash");

        generator.handle_schema_const(&token, &mut buffer);
        assert_eq!(buffer.len(), 1);
        assert_eq!(
            buffer[0],
            "export const schema: SchemaType = {\n\tonchain_dash: {\n\t\tTestStruct: \
             {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t},\n};"
        );

        let token_2 = create_test_struct_token("AvailableTheme", "onchain_dash");
        generator.handle_schema_const(&token_2, &mut buffer);
        assert_eq!(buffer.len(), 1);
        assert_eq!(
            buffer[0],
            "export const schema: SchemaType = {\n\tonchain_dash: {\n\t\tTestStruct: \
             {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t\tAvailableTheme: \
             {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t},\n};"
        );

        let token_3 = create_test_struct_token("Player", "combat");
        generator.handle_schema_const(&token_3, &mut buffer);
        assert_eq!(buffer.len(), 1);
        assert_eq!(
            buffer[0],
            "export const schema: SchemaType = {\n\tonchain_dash: {\n\t\tTestStruct: \
             {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t\tAvailableTheme: \
             {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t},\n\tcombat: \
             {\n\t\tPlayer: {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: \
             0,\n\t\t},\n\t},\n};"
        );

        let token_4 = create_test_struct_token("Position", "combat");
        generator.handle_schema_const(&token_4, &mut buffer);
        assert_eq!(buffer.len(), 1);
        assert_eq!(
            buffer[0],
            "export const schema: SchemaType = {\n\tonchain_dash: {\n\t\tTestStruct: \
             {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t\tAvailableTheme: \
             {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t},\n\tcombat: \
             {\n\t\tPlayer: {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: \
             0,\n\t\t},\n\t\tPosition: {\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: \
             0,\n\t\t},\n\t},\n};"
        );
    }

    #[test]
    fn test_handle_nested_struct() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Buffer::new();
        let nested_struct = create_test_nested_struct_token("TestNestedStruct", "onchain_dash");
        let _res = generator.generate(&nested_struct, &mut buffer);
        assert_eq!(buffer.len(), 3);
    }

    fn create_test_struct_token(name: &str, namespace: &str) -> Composite {
        Composite {
            type_path: format!("{namespace}::{name}"),
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

    #[test]
    fn test_imported_models_use_world_namespace() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Buffer::new();

        // Add world name marker
        buffer.push("// WORLD_NAME:myworld".to_string());

        // Create a model with different namespace (simulating imported model)
        let imported_model = create_test_struct_token("Achievement", "arcade");
        generator.handle_schema_type(&imported_model, &mut buffer);

        // Should use world namespace "myworld" instead of "arcade"
        assert!(buffer.has("\tmyworld: {\n\t\tAchievement: Achievement,"));
        assert!(!buffer.has("\tarcade: {"));
    }

    #[test]
    fn test_imported_models_use_world_namespace_in_const() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Buffer::new();

        // Add world name marker
        buffer.push("// WORLD_NAME:myworld".to_string());

        // Create a model with different namespace (simulating imported model)
        let imported_model = create_test_struct_token("Achievement", "arcade");
        generator.handle_schema_const(&imported_model, &mut buffer);

        // Should use world namespace "myworld" instead of "arcade" in const
        assert!(buffer.has("\tmyworld: {\n\t\tAchievement: {"));
        assert!(!buffer.has("\tarcade: {"));
    }

    pub fn create_test_nested_struct_token(name: &str, namespace: &str) -> Composite {
        Composite {
            type_path: format!("{namespace}::{name}"),
            inners: vec![
                CompositeInner {
                    index: 0,
                    name: "field1".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::Array(cainome::parser::tokens::Array {
                        type_path: format!("core::array::Array::<{namespace}::Direction>"),
                        inner: Box::new(Token::Composite(Composite {
                            type_path: format!("{namespace}::Direction"),
                            inners: vec![
                                CompositeInner {
                                    index: 0,
                                    name: "None".to_owned(),
                                    kind: CompositeInnerKind::Key,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::fetl252".to_owned(),
                                    }),
                                },
                                CompositeInner {
                                    index: 1,
                                    name: "North".to_owned(),
                                    kind: CompositeInnerKind::Key,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::fetl252".to_owned(),
                                    }),
                                },
                                CompositeInner {
                                    index: 2,
                                    name: "South".to_owned(),
                                    kind: CompositeInnerKind::Key,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::fetl252".to_owned(),
                                    }),
                                },
                                CompositeInner {
                                    index: 3,
                                    name: "West".to_owned(),
                                    kind: CompositeInnerKind::Key,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::fetl252".to_owned(),
                                    }),
                                },
                                CompositeInner {
                                    index: 4,
                                    name: "East".to_owned(),
                                    kind: CompositeInnerKind::Key,
                                    token: Token::CoreBasic(CoreBasic {
                                        type_path: "core::fetl252".to_owned(),
                                    }),
                                },
                            ],
                            generic_args: vec![],
                            r#type: CompositeType::Enum,
                            is_event: false,
                            alias: None,
                        })),
                        is_legacy: false,
                    }),
                },
                CompositeInner {
                    index: 1,
                    name: "field2".to_owned(),
                    kind: CompositeInnerKind::Key,
                    token: Token::Composite(create_test_struct_token("Position", namespace)),
                },
            ],
            generic_args: vec![],
            r#type: CompositeType::Struct,
            is_event: false,
            alias: None,
        }
    }
}
