use cainome::parser::tokens::{CompositeType, Function, Token};
use convert_case::{Case, Casing};
use dojo_world::contracts::naming;

use super::JsType;
use crate::error::BindgenResult;
use crate::plugins::{BindgenContractGenerator, Buffer};
use crate::DojoContract;

pub(crate) struct TsFunctionGenerator;
impl TsFunctionGenerator {
    fn check_imports(&self, buffer: &mut Buffer) {
        if !buffer.has("import { DojoProvider } from ") {
            buffer.insert(0, "import { DojoProvider } from \"@dojoengine/core\";".to_owned());
            buffer.insert(1, "import { Account } from \"starknet\";".to_owned());
            buffer.insert(2, "import * as models from \"./models.gen\";\n".to_owned());
        }
    }

    fn setup_function_wrapper_start(&self, buffer: &mut Buffer) -> usize {
        let fn_wrapper = "export async function setupWorld(provider: DojoProvider) {\n";

        if !buffer.has(fn_wrapper) {
            buffer.push(fn_wrapper.to_owned());
        }

        buffer.iter().position(|b| b.contains(fn_wrapper)).unwrap()
    }

    fn generate_system_function(
        &self,
        namespace: &str,
        contract_name: &str,
        token: &Function,
    ) -> String {
        format!(
            "\tconst {contract_name}_{} = async ({}) => {{
\t\ttry {{
\t\t\treturn await provider.execute(
\t\t\t\tsnAccount,
\t\t\t\t{{
\t\t\t\t\tcontractName: \"{contract_name}\",
\t\t\t\t\tentrypoint: \"{}\",
\t\t\t\t\tcalldata: [{}],
\t\t\t\t}},
\t\t\t\t\"{namespace}\",
\t\t\t);
\t\t}} catch (error) {{
\t\t\tconsole.error(error);
\t\t}}
\t}};\n",
            token.name.to_case(Case::Camel),
            self.format_function_inputs(token),
            token.name,
            self.format_function_calldata(token)
        )
    }

    fn format_function_inputs(&self, token: &Function) -> String {
        let inputs = vec!["snAccount: Account".to_owned()];
        token
            .inputs
            .iter()
            .fold(inputs, |mut acc, input| {
                let prefix = match &input.1 {
                    Token::Composite(t) => {
                        if t.r#type == CompositeType::Enum
                            || (t.r#type == CompositeType::Struct
                                && !t.type_path.starts_with("core"))
                        {
                            "models."
                        } else {
                            ""
                        }
                    }
                    _ => "",
                };
                acc.push(format!(
                    "{}: {}{}",
                    input.0.to_case(Case::Camel),
                    prefix,
                    JsType::from(&input.1)
                ));
                acc
            })
            .join(", ")
    }

    fn format_function_calldata(&self, token: &Function) -> String {
        token
            .inputs
            .iter()
            .fold(Vec::new(), |mut acc, input| {
                acc.push(input.0.to_case(Case::Camel));
                acc
            })
            .join(", ")
    }

    fn append_function_body(&self, idx: usize, buffer: &mut Buffer, body: String) {
        // check if functions was already appended to body, if so, append after other functions
        let pos = if buffer.len() - idx > 2 { buffer.len() - 2 } else { idx };

        buffer.insert(pos + 1, body);
    }

    fn setup_function_wrapper_end(
        &self,
        contract_name: &str,
        token: &Function,
        buffer: &mut Buffer,
    ) {
        let return_token = "\treturn {";
        if !buffer.has(return_token) {
            buffer.push(format!(
                "\treturn {{\n\t\t{}: {{\n\t\t\t{}: {}_{},\n\t\t}},\n\t}};\n}}",
                contract_name,
                token.name.to_case(Case::Camel),
                contract_name,
                token.name.to_case(Case::Camel)
            ));
            return;
        }

        // if buffer has return and has contract_name, we append in this object if contract_name is
        // the same

        let contract_name_token = format!("\n\t\t{}: {{\n\t\t\t", contract_name);
        if buffer.has(contract_name_token.as_str()) {
            // we can expect safely as condition has is true there
            let return_idx = buffer.pos(return_token).expect("return token not found");
            // find closing curly bracket to get closing of object `contract_name`, get the last
            // comma and insert token just after it.
            if let Some(pos) = buffer.get_first_after(
                format!("\n\t\t{}: {{\n\t\t\t", contract_name).as_str(),
                "}",
                return_idx,
            ) {
                if let Some(insert_pos) = buffer.get_first_before_pos(",", pos, return_idx) {
                    let gen = format!(
                        "\n\t\t\t{}: {}_{},",
                        token.name.to_case(Case::Camel),
                        contract_name,
                        token.name.to_case(Case::Camel)
                    );
                    // avoid duplicating identifiers. This can occur because some contracts keeps
                    // camel ans snake cased functions
                    if !buffer.has(&gen) {
                        buffer.insert_at(gen, insert_pos, return_idx);
                    }
                    return;
                }
            }
        }

        // if buffer has return but not contract_name, we append in this object
        buffer.insert_after(
            format!(
                "\n\t\t{}: {{\n\t\t\t{}: {}_{},\n\t\t}},",
                contract_name,
                token.name.to_case(Case::Camel),
                contract_name,
                token.name.to_case(Case::Camel),
            ),
            return_token,
            ",",
            1,
        );
    }
}

impl BindgenContractGenerator for TsFunctionGenerator {
    fn generate(
        &self,
        contract: &DojoContract,
        token: &Function,
        buffer: &mut Buffer,
    ) -> BindgenResult<String> {
        self.check_imports(buffer);
        let contract_name = naming::get_name_from_tag(&contract.tag);
        let idx = self.setup_function_wrapper_start(buffer);

        // avoid duplicating function body that can occur because some contracts keeps camel and
        // snake cased identifiers
        let fn_body = self.generate_system_function(
            &naming::get_namespace_from_tag(&contract.tag),
            contract_name.as_str(),
            token,
        );
        if let Some(fn_idx) = fn_body.chars().position(|c| c == '=') {
            let fn_name = fn_body[0..fn_idx].to_string();
            if !buffer.has(&fn_name) {
                self.append_function_body(idx, buffer, fn_body);
            }
        }

        self.setup_function_wrapper_end(contract_name.as_str(), token, buffer);
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use cainome::parser::tokens::{CoreBasic, Function, Token};
    use cainome::parser::TokenizedAbi;
    use dojo_world::contracts::naming;

    use super::TsFunctionGenerator;
    use crate::plugins::{BindgenContractGenerator, Buffer};
    use crate::DojoContract;

    #[test]
    fn test_check_imports() {
        let generator = TsFunctionGenerator {};
        let mut buff = Buffer::new();

        // check imports are added only once
        generator.check_imports(&mut buff);
        assert_eq!(buff.len(), 3);
        generator.check_imports(&mut buff);
        assert_eq!(buff.len(), 3);
    }

    #[test]
    fn test_setup_function_wrapper_start() {
        let generator = TsFunctionGenerator {};
        let mut buff = Buffer::new();
        let idx = generator.setup_function_wrapper_start(&mut buff);

        assert_eq!(buff.len(), 1);
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_generate_system_function() {
        let generator = TsFunctionGenerator {};
        let function = create_change_theme_function();
        let expected = "\tconst actions_changeTheme = async (snAccount: Account, value: number) \
                        => {
\t\ttry {
\t\t\treturn await provider.execute(
\t\t\t\tsnAccount,
\t\t\t\t{
\t\t\t\t\tcontractName: \"actions\",
\t\t\t\t\tentrypoint: \"change_theme\",
\t\t\t\t\tcalldata: [value],
\t\t\t\t},
\t\t\t\t\"onchain_dash\",
\t\t\t);
\t\t} catch (error) {
\t\t\tconsole.error(error);
\t\t}
\t};
";

        let contract = create_dojo_contract();
        assert_eq!(
            expected,
            generator.generate_system_function(
                naming::get_namespace_from_tag(&contract.tag).as_str(),
                naming::get_name_from_tag(&contract.tag).as_str(),
                &function
            )
        )
    }

    #[test]
    fn test_format_function_inputs() {
        let generator = TsFunctionGenerator {};
        let function = create_change_theme_function();
        let expected = "snAccount: Account, value: number";
        assert_eq!(expected, generator.format_function_inputs(&function))
    }

    #[test]
    fn test_format_function_inputs_complex() {
        let generator = TsFunctionGenerator {};
        let function = create_change_theme_function();
        let expected = "snAccount: Account, value: number";
        assert_eq!(expected, generator.format_function_inputs(&function))
    }

    #[test]
    fn test_format_function_calldata() {
        let generator = TsFunctionGenerator {};
        let function = create_change_theme_function();
        let expected = "value";
        assert_eq!(expected, generator.format_function_calldata(&function))
    }

    #[test]
    fn test_append_function_body() {
        let generator = TsFunctionGenerator {};
        let mut buff = Buffer::new();
        buff.push("import".to_owned());
        buff.push("function wrapper".to_owned());

        generator.append_function_body(1, &mut buff, "function body".to_owned());

        assert_eq!(buff[2], "function body".to_owned());
    }

    #[test]
    fn test_setup_function_wrapper_end() {
        let generator = TsFunctionGenerator {};
        let mut buff = Buffer::new();

        generator.setup_function_wrapper_end("actions", &create_change_theme_function(), &mut buff);

        let expected = "\treturn {
\t\tactions: {
\t\t\tchangeTheme: actions_changeTheme,
\t\t},
\t};
}";

        assert_eq!(1, buff.len());
        assert_eq!(expected, buff[0]);

        generator.setup_function_wrapper_end(
            "actions",
            &create_increate_global_counter_function(),
            &mut buff,
        );
        let expected_2 = "\treturn {
\t\tactions: {
\t\t\tchangeTheme: actions_changeTheme,
\t\t\tincreaseGlobalCounter: actions_increaseGlobalCounter,
\t\t},
\t};
}";
        assert_eq!(1, buff.len());
        assert_eq!(expected_2, buff[0]);

        generator.setup_function_wrapper_end("dojo_starter", &create_move_function(), &mut buff);
        let expected_3 = "\treturn {
\t\tactions: {
\t\t\tchangeTheme: actions_changeTheme,
\t\t\tincreaseGlobalCounter: actions_increaseGlobalCounter,
\t\t},
\t\tdojo_starter: {
\t\t\tmove: dojo_starter_move,
\t\t},
\t};
}";
        assert_eq!(1, buff.len());
        assert_eq!(expected_3, buff[0]);
    }

    #[test]
    fn test_setup_function_wrapper_end_does_not_duplicate_identifiers() {
        let generator = TsFunctionGenerator {};
        let mut buff = Buffer::new();

        generator.setup_function_wrapper_end("actions", &create_change_theme_function(), &mut buff);

        let expected = "\treturn {
\t\tactions: {
\t\t\tchangeTheme: actions_changeTheme,
\t\t},
\t};
}";
        assert_eq!(1, buff.len());
        assert_eq!(expected, buff[0]);
        generator.setup_function_wrapper_end("actions", &create_change_theme_function(), &mut buff);
        assert_eq!(1, buff.len());
        assert_eq!(expected, buff[0]);
    }

    #[test]
    fn test_it_generates_function() {
        let generator = TsFunctionGenerator {};
        let mut buffer = Buffer::new();
        let change_theme = create_change_theme_function();

        let onchain_dash_contract = create_dojo_contract();
        let _ = generator.generate(&onchain_dash_contract, &change_theme, &mut buffer);

        assert_eq!(buffer.len(), 6);
        let increase_global_counter = create_increate_global_counter_function();
        let _ = generator.generate(&onchain_dash_contract, &increase_global_counter, &mut buffer);
        assert_eq!(buffer.len(), 7);
    }

    #[test]
    fn test_generate() {
        let generator = TsFunctionGenerator {};
        let mut buff = Buffer::new();
        let function = create_change_theme_function();
        let function_ca = create_change_theme_function_camelized();

        let contract = create_dojo_contract();

        let _ = generator.generate(&contract, &function, &mut buff);
        assert_eq!(6, buff.len());
        let _ = generator.generate(&contract, &function_ca, &mut buff);
        assert_eq!(6, buff.len());
    }

    fn create_change_theme_function() -> Function {
        create_test_function(
            "change_theme",
            vec![(
                "value".to_owned(),
                Token::CoreBasic(CoreBasic { type_path: "core::integer::u8".to_owned() }),
            )],
        )
    }
    fn create_change_theme_function_camelized() -> Function {
        create_test_function(
            "changeTheme",
            vec![(
                "value".to_owned(),
                Token::CoreBasic(CoreBasic { type_path: "core::integer::u8".to_owned() }),
            )],
        )
    }

    fn create_increate_global_counter_function() -> Function {
        create_test_function("increase_global_counter", vec![])
    }
    fn create_move_function() -> Function {
        create_test_function("move", vec![])
    }

    fn create_test_function(name: &str, inputs: Vec<(String, Token)>) -> Function {
        Function {
            name: name.to_owned(),
            state_mutability: cainome::parser::tokens::StateMutability::External,
            inputs,
            outputs: vec![],
            named_outputs: vec![],
        }
    }

    fn create_dojo_contract() -> DojoContract {
        DojoContract {
            tag: "onchain_dash-actions".to_owned(),
            tokens: TokenizedAbi::default(),
            systems: vec![],
        }
    }
}
