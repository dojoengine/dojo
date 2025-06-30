use cainome::parser::tokens::{CompositeType, Function, StateMutability, Token};
use convert_case::{Case, Casing};
use dojo_world::contracts::naming;

use super::constants::JS_BIGNUMBERISH;
use super::{token_is_enum, JsPrimitiveInputType};
use crate::error::BindgenResult;
use crate::plugins::{BindgenContractGenerator, Buffer};
use crate::DojoContract;

pub(crate) struct TsFunctionGenerator;
impl TsFunctionGenerator {
    fn check_imports(&self, buffer: &mut Buffer) {
        if !buffer.has("import { DojoProvider, DojoCall } from ") {
            buffer.insert(
                0,
                "import { DojoProvider, DojoCall } from \"@dojoengine/core\";".to_owned(),
            );
            buffer.insert(
                1,
                format!(
                    "import {{ Account, AccountInterface, {}, CairoOption, CairoCustomEnum }} \
                     from \"starknet\";",
                    JS_BIGNUMBERISH
                ),
            );
            buffer.insert(2, "import * as models from \"./models.gen\";\n".to_owned());
        }
    }

    fn setup_function_wrapper_start(&self, buffer: &mut Buffer) -> usize {
        let fn_wrapper = "export function setupWorld(provider: DojoProvider) {\n";

        if !buffer.has(fn_wrapper) {
            buffer.push(fn_wrapper.to_owned());
        }

        buffer.iter().position(|b| b.contains(fn_wrapper)).unwrap()
    }

    /// Generate string template to build function calldata.
    /// * namespace - &str - Token namespace
    /// * contract_name - &str - Token contract_name avoid name clashing between different systems
    /// * token - &Function - cairo function token
    fn build_function_calldata(&self, contract_name: &str, token: &Function) -> String {
        format!(
            "\tconst build_{contract_name}_{}_calldata = ({}): DojoCall => {{
\t\treturn {{
\t\t\tcontractName: \"{contract_name}\",
\t\t\tentrypoint: \"{}\",
\t\t\tcalldata: [{}],
\t\t}};
\t}};
",
            token.name.to_case(Case::Camel),
            self.get_function_input_args(token).join(", "),
            token.name,
            self.format_function_calldata(token),
        )
    }

    /// Generate string template to build system function
    /// We call different methods depending if token is View or External
    /// * namespace - &str - Token namespace
    /// * contract_name - &str - Token contract_name avoid name clashing between different systems
    /// * token - &Function - cairo function token
    fn generate_system_function(
        &self,
        namespace: &str,
        contract_name: &str,
        token: &Function,
    ) -> String {
        let function_calldata = self.build_function_calldata(contract_name, token);
        let function_name = format!("{contract_name}_{}", token.name.to_case(Case::Camel));
        let function_call = match token.state_mutability {
            StateMutability::External => format!(
                "\tconst {function_name} = async ({}) => {{
\t\ttry {{
\t\t\treturn await provider.execute(
\t\t\t\tsnAccount,
\t\t\t\tbuild_{function_name}_calldata({}),
\t\t\t\t\"{namespace}\",
\t\t\t);
\t\t}} catch (error) {{
\t\t\tconsole.error(error);
\t\t\tthrow error;
\t\t}}
\t}};\n",
                self.format_function_inputs(token),
                self.format_function_calldata(token),
            ),
            StateMutability::View => format!(
                "\tconst {function_name} = async ({}) => {{
\t\ttry {{
\t\t\treturn await provider.call(\"{namespace}\", build_{function_name}_calldata({}));
\t\t}} catch (error) {{
\t\t\tconsole.error(error);
\t\t\tthrow error;
\t\t}}
\t}};\n",
                self.format_function_inputs(token),
                self.format_function_calldata(token),
            ),
        };
        format!("{}\n{}", function_calldata, function_call)
    }

    fn get_function_input_args(&self, token: &Function) -> Vec<String> {
        token.inputs.iter().fold(Vec::new(), |mut acc, input| {
            let prefix = match &input.1 {
                Token::Composite(t) => {
                    if !token_is_enum(t)
                        && (t.r#type == CompositeType::Enum
                            || (t.r#type == CompositeType::Struct
                                && !t.type_path.starts_with("core"))
                            || t.r#type == CompositeType::Unknown)
                    {
                        "models."
                    } else {
                        ""
                    }
                }
                _ => "",
            };
            let mut type_input = JsPrimitiveInputType::from(&input.1).to_string();
            if type_input.contains("<") {
                type_input = type_input.replace("<", format!("<{}", prefix).as_str());
            } else {
                type_input = format!("{}{}", prefix, type_input);
            }
            acc.push(format!("{}: {}", input.0.to_case(Case::Camel), type_input));
            acc
        })
    }

    fn format_function_inputs(&self, token: &Function) -> String {
        let inputs = match token.state_mutability {
            StateMutability::External => vec!["snAccount: Account | AccountInterface".to_owned()],
            StateMutability::View => Vec::new(),
        };
        [inputs, self.get_function_input_args(token)].concat().join(", ")
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
        let function_name = format!("{contract_name}_{}", token.name.to_case(Case::Camel));
        let build_calldata = format!("build{}Calldata", token.name.to_case(Case::Pascal));
        let build_calldata_sc =
            format!("build_{contract_name}_{}_calldata", token.name.to_case(Case::Camel));
        let return_token = "\n\n\treturn {";
        if !buffer.has(return_token) {
            buffer.push(format!(
                "\n\n\treturn {{\n\t\t{}: {{\n\t\t\t{}: {function_name},\n\t\t\t{build_calldata}: \
                 {build_calldata_sc},\n\t\t}},\n\t}};\n}}",
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
                        "\n\t\t\t{}: {function_name},\n\t\t\t{build_calldata}: \
                         {build_calldata_sc},",
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
                "\n\t\t{}: {{\n\t\t\t{}: {function_name},\n\t\t\t{build_calldata}: \
                 {build_calldata_sc},\n\t\t}},",
                contract_name,
                token.name.to_case(Case::Camel)
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
    use cainome::parser::tokens::{
        Array, Composite, CompositeInner, CompositeInnerKind, CompositeType, CoreBasic, Function,
        StateMutability, Token,
    };
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
        let expected = "\tconst build_actions_changeTheme_calldata = (value: BigNumberish): \
                        DojoCall => {
\t\treturn {
\t\t\tcontractName: \"actions\",
\t\t\tentrypoint: \"change_theme\",
\t\t\tcalldata: [value],
\t\t};
\t};

\tconst actions_changeTheme = async (snAccount: Account | AccountInterface, value: BigNumberish) \
                        => {
\t\ttry {
\t\t\treturn await provider.execute(
\t\t\t\tsnAccount,
\t\t\t\tbuild_actions_changeTheme_calldata(value),
\t\t\t\t\"onchain_dash\",
\t\t\t);
\t\t} catch (error) {
\t\t\tconsole.error(error);
\t\t\tthrow error;
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
    fn test_generate_system_function_view() {
        let generator = TsFunctionGenerator {};
        let function = create_change_theme_view_function();
        let expected = "\tconst build_actions_changeTheme_calldata = (value: BigNumberish): \
                        DojoCall => {
\t\treturn {
\t\t\tcontractName: \"actions\",
\t\t\tentrypoint: \"change_theme\",
\t\t\tcalldata: [value],
\t\t};
\t};

\tconst actions_changeTheme = async (value: BigNumberish) => {
\t\ttry {
\t\t\treturn await provider.call(\"onchain_dash\", build_actions_changeTheme_calldata(value));
\t\t} catch (error) {
\t\t\tconsole.error(error);
\t\t\tthrow error;
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
        let expected = "snAccount: Account | AccountInterface, value: BigNumberish";
        assert_eq!(expected, generator.format_function_inputs(&function))
    }
    #[test]
    fn test_format_function_inputs_view() {
        let generator = TsFunctionGenerator {};
        let function = create_basic_view_function();
        let expected = "";
        assert_eq!(expected, generator.format_function_inputs(&function))
    }

    #[test]
    fn test_format_function_inputs_complex() {
        let generator = TsFunctionGenerator {};
        let function = create_change_theme_function();
        let expected = "snAccount: Account | AccountInterface, value: BigNumberish";
        assert_eq!(expected, generator.format_function_inputs(&function))
    }

    #[test]
    fn test_format_function_inputs_cairo_option() {
        let generator = TsFunctionGenerator {};
        let function = create_function_with_option_param();
        let expected =
            "snAccount: Account | AccountInterface, value: CairoOption<models.GatedType>";
        assert_eq!(expected, generator.format_function_inputs(&function))
    }

    #[test]
    fn test_format_function_inputs_cairo_enum() {
        let generator = TsFunctionGenerator {};
        let function = create_function_with_custom_enum();
        let expected = "snAccount: Account | AccountInterface, value: models.GatedType";
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

        let expected = "\n\n\treturn {
\t\tactions: {
\t\t\tchangeTheme: actions_changeTheme,
\t\t\tbuildChangeThemeCalldata: build_actions_changeTheme_calldata,
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
        let expected_2 = "\n\n\treturn {
\t\tactions: {
\t\t\tchangeTheme: actions_changeTheme,
\t\t\tbuildChangeThemeCalldata: build_actions_changeTheme_calldata,
\t\t\tincreaseGlobalCounter: actions_increaseGlobalCounter,
\t\t\tbuildIncreaseGlobalCounterCalldata: build_actions_increaseGlobalCounter_calldata,
\t\t},
\t};
}";
        assert_eq!(1, buff.len());
        assert_eq!(expected_2, buff[0]);

        generator.setup_function_wrapper_end("dojo_starter", &create_move_function(), &mut buff);
        let expected_3 = "\n\n\treturn {
\t\tactions: {
\t\t\tchangeTheme: actions_changeTheme,
\t\t\tbuildChangeThemeCalldata: build_actions_changeTheme_calldata,
\t\t\tincreaseGlobalCounter: actions_increaseGlobalCounter,
\t\t\tbuildIncreaseGlobalCounterCalldata: build_actions_increaseGlobalCounter_calldata,
\t\t},
\t\tdojo_starter: {
\t\t\tmove: dojo_starter_move,
\t\t\tbuildMoveCalldata: build_dojo_starter_move_calldata,
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

        let expected = "\n\n\treturn {
\t\tactions: {
\t\t\tchangeTheme: actions_changeTheme,
\t\t\tbuildChangeThemeCalldata: build_actions_changeTheme_calldata,
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
            StateMutability::External,
        )
    }

    fn create_change_theme_view_function() -> Function {
        create_test_function(
            "change_theme",
            vec![(
                "value".to_owned(),
                Token::CoreBasic(CoreBasic { type_path: "core::integer::u8".to_owned() }),
            )],
            StateMutability::View,
        )
    }

    fn create_basic_view_function() -> Function {
        Function {
            name: "allowance".to_owned(),
            state_mutability: cainome::parser::tokens::StateMutability::View,
            inputs: vec![],
            outputs: vec![],
            named_outputs: vec![],
        }
    }

    fn create_change_theme_function_camelized() -> Function {
        create_test_function(
            "changeTheme",
            vec![(
                "value".to_owned(),
                Token::CoreBasic(CoreBasic { type_path: "core::integer::u8".to_owned() }),
            )],
            StateMutability::External,
        )
    }

    fn create_increate_global_counter_function() -> Function {
        create_test_function("increase_global_counter", vec![], StateMutability::External)
    }

    fn create_move_function() -> Function {
        create_test_function("move", vec![], StateMutability::External)
    }

    fn create_test_function(
        name: &str,
        inputs: Vec<(String, Token)>,
        state_mutability: StateMutability,
    ) -> Function {
        Function {
            name: name.to_owned(),
            state_mutability,
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

    fn create_function_with_option_param() -> Function {
        Function {
            name: "cairo_option".to_owned(),
            state_mutability: cainome::parser::tokens::StateMutability::External,
            inputs: vec![("value".to_owned(), Token::Composite(Composite {
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
                alias: None
            }))],
            outputs: vec![],
            named_outputs: vec![],
        }
    }
    fn create_function_with_custom_enum() -> Function {
        Function {
            name: "cairo_enum".to_owned(),
            state_mutability: cainome::parser::tokens::StateMutability::External,
            inputs: vec![("value".to_owned(), Token::Composite(Composite {
                type_path: "tournament::ls15_components::models::tournament::GatedType".to_owned(),
                inners: vec![
                    CompositeInner {
                        index: 0,
                        name: "token".to_owned(),
                        kind: CompositeInnerKind::NotUsed,
                        token: Token::Composite(
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
                            }),
                    }
                ],
                generic_args: vec![],
                r#type: CompositeType::Unknown,
                is_event: false,
                alias: None
            }))],
            outputs: vec![],
            named_outputs: vec![],
        }
    }
}
