use cainome::parser::tokens::{Composite, CompositeType};
use convert_case::{Case, Casing};

use crate::{
    error::BindgenResult,
    plugins::{typescript::generator::JsDefaultValue, BindgenModelGenerator},
};

/// This generator will build a schema based on previous generator results.
/// first we need to generate interface of schema which will be used in dojo.js sdk to fully type
/// sdk
/// then we need to build the schema const which contains default values for all fieleds
pub(crate) struct TsSchemaGenerator {}
impl TsSchemaGenerator {
    /// Import only needs to be present once
    fn import_schema_type(&self, buffer: &mut Vec<String>) {
        if !buffer.iter().any(|b| b.contains("import type { SchemaType }")) {
            buffer.insert(0, "import type { SchemaType } from \"@dojoengine/sdk\";\n".to_owned());
        }
    }

    fn get_namespace_and_path(&self, token: &Composite) -> (String, String, String) {
        let ns_split = token.type_path.split("::").collect::<Vec<&str>>();
        if ns_split.len() < 2 {
            panic!("type is invalid type_path has to be at least namespace::type");
        }
        let ns = ns_split[0];
        let type_name = ns_split[ns_split.len() - 1];
        let namespace = ns.to_case(Case::Pascal);
        (ns.to_owned(), namespace, type_name.to_owned())
    }

    /// Generates the type definition for the schema
    fn handle_schema_type(&self, token: &Composite, buffer: &mut Vec<String>) {
        let (ns, namespace, type_name) = self.get_namespace_and_path(token);
        let schema_type = format!("export interface {namespace}SchemaType extends SchemaType");
        if !buffer.iter().any(|b| b.contains(&schema_type)) {
            buffer.push(format!("export interface {namespace}SchemaType extends SchemaType {{\n\t{ns}: {{\n\t\t{}: {},\n\t}},\n}}", type_name, type_name));
            return;
        }

        // type has already been initialized
        let gen = format!("\n\t\t{type_name}: {type_name},");
        if buffer.iter().any(|b| b.contains(&gen)) {
            return;
        }

        // fastest way to add a field to the interface is to search for the n-1 `,` and add the
        // field directly after it.
        // to improve this logic, we would need to either have some kind of code parsing.
        // we could otherwise have some intermediate representation that we pass to this generator
        // function.
        let pos = buffer.iter().position(|b| b.contains(&schema_type)).unwrap();
        if let Some(st) = buffer.get_mut(pos) {
            let indices = st.match_indices(",").map(|(i, _)| i).collect::<Vec<usize>>();
            let append_after = indices[indices.len() - 2] + 1;
            st.insert_str(append_after, &gen);
        }
    }

    /// Generates the default values for the schema
    fn handle_schema_const(&self, token: &Composite, buffer: &mut Vec<String>) {
        let (ns, namespace, type_name) = self.get_namespace_and_path(token);
        let const_type = format!("export const schema: {namespace}SchemaType");
        if !buffer.iter().any(|b| b.contains(&const_type)) {
            buffer.push(format!(
            "export const schema: {namespace}SchemaType = {{\n\t{ns}: {{\n\t\t{}: {},\n\t}},\n}};",
            type_name,
            self.generate_type_init(token)
            ));
            return;
        }

        // type has already been initialized
        let gen = format!("\n\t\t{type_name}: {},", self.generate_type_init(token));
        if buffer.iter().any(|b| b.contains(&gen)) {
            return;
        }

        let pos = buffer.iter().position(|b| b.contains(&const_type)).unwrap();
        if let Some(st) = buffer.get_mut(pos) {
            let indices = st.match_indices(",").map(|(i, _)| i).collect::<Vec<usize>>();
            let append_after = indices[indices.len() - 2] + 1;
            st.insert_str(append_after, &gen);
        }
    }

    /// Generates default values for each fields of the struct.
    fn generate_type_init(&self, token: &Composite) -> String {
        format!(
            "{{\n\t\t\tfieldOrder: [{}],\n{}\n\t\t}}",
            token
                .inners
                .iter()
                .map(|i| format!("'{}'", i.name))
                .collect::<Vec<String>>()
                .join(", "),
            token
                .inners
                .iter()
                .map(|i| {
                    match i.token.to_composite() {
                        Ok(c) => {
                            format!("\t\t\t{}: {},", i.name, JsDefaultValue::from(c))
                        }
                        Err(_) => {
                            // this will fail on core types as `core::starknet::contract_address::ContractAddress`
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
}

impl BindgenModelGenerator for TsSchemaGenerator {
    fn generate(&self, token: &Composite, buffer: &mut Vec<String>) -> BindgenResult<String> {
        if token.inners.is_empty() || token.r#type != CompositeType::Struct {
            return Ok(String::new());
        }
        self.import_schema_type(buffer);

        // in buffer search for interface named {pascal_case(namespace)}SchemaType extends
        // SchemaType
        // this should be hold in a buffer item
        self.handle_schema_type(token, buffer);

        // in buffer search for const schema: InterfaceName =  named {pascal_case(namespace)}SchemaType extends
        // SchemaType
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
        let mut buffer = Vec::new();

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
        let mut buffer = Vec::new();

        let token = create_test_struct_token("TestStruct");
        let _result = generator.generate(&token, &mut buffer);

        // token is not empty, we should have an import
        assert_eq!("import type { SchemaType } from \"@dojoengine/sdk\";\n", buffer[0]);
    }

    /// NOTE: For the following tests, we assume that the `enum.rs` and `interface.rs` generators
    /// successfully ran and generated related output to generator base interfaces + enums.
    #[test]
    fn test_it_appends_schema_type() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Vec::new();

        let token = create_test_struct_token("TestStruct");
        let _result = generator.generate(&token, &mut buffer);
        assert_eq!("export interface OnchainDashSchemaType extends SchemaType {\n\tonchain_dash: {\n\t\tTestStruct: TestStruct,\n\t},\n}", buffer[1]);
    }

    #[test]
    fn test_handle_schema_type() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Vec::new();

        let token = create_test_struct_token("TestStruct");
        generator.handle_schema_type(&token, &mut buffer);

        assert_ne!(0, buffer.len());
        assert_eq!("export interface OnchainDashSchemaType extends SchemaType {\n\tonchain_dash: {\n\t\tTestStruct: TestStruct,\n\t},\n}", buffer[0]);

        let token_2 = create_test_struct_token("AvailableTheme");
        generator.handle_schema_type(&token_2, &mut buffer);
        assert_eq!("export interface OnchainDashSchemaType extends SchemaType {\n\tonchain_dash: {\n\t\tTestStruct: TestStruct,\n\t\tAvailableTheme: AvailableTheme,\n\t},\n}", buffer[0]);
    }

    #[test]
    fn test_generate_type_init() {
        let generator = TsSchemaGenerator {};
        let token = create_test_struct_token("TestStruct");
        let init_type = generator.generate_type_init(&token);

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
    fn test_handle_schema_const() {
        let generator = TsSchemaGenerator {};
        let mut buffer = Vec::new();
        let token = create_test_struct_token("TestStruct");

        generator.handle_schema_const(&token, &mut buffer);
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0], "export const schema: OnchainDashSchemaType = {\n\tonchain_dash: {\n\t\tTestStruct: {\n\t\t\tfieldOrder: ['field1', 'field2', 'field3'],\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t},\n};");

        let token_2 = create_test_struct_token("AvailableTheme");
        generator.handle_schema_const(&token_2, &mut buffer);
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0], "export const schema: OnchainDashSchemaType = {\n\tonchain_dash: {\n\t\tTestStruct: {\n\t\t\tfieldOrder: ['field1', 'field2', 'field3'],\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t\tAvailableTheme: {\n\t\t\tfieldOrder: ['field1', 'field2', 'field3'],\n\t\t\tfield1: 0,\n\t\t\tfield2: 0,\n\t\t\tfield3: 0,\n\t\t},\n\t},\n};");
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
