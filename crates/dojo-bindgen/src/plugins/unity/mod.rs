use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use cainome::parser::tokens::{Composite, CompositeType, Function, FunctionOutputKind, Token};

use crate::error::BindgenResult;
use crate::plugins::BuiltinPlugin;
use crate::{DojoContract, DojoData, DojoModel};

pub struct UnityPlugin {}

impl UnityPlugin {
    pub fn new() -> Self {
        Self {}
    }

    // Maps cairo types to C#/Unity SDK defined types
    fn map_type(token: &Token) -> String {
        match token.type_name().as_str() {
            "u8" => "byte".to_string(),
            "u16" => "ushort".to_string(),
            "u32" => "uint".to_string(),
            "u64" => "ulong".to_string(),
            "u128" => "BigInteger".to_string(),
            "u256" => "BigInteger".to_string(),
            "usize" => "uint".to_string(),
            "felt252" => "FieldElement".to_string(),
            "bytes31" => "string".to_string(),
            "ClassHash" => "FieldElement".to_string(),
            "ContractAddress" => "FieldElement".to_string(),
            "ByteArray" => "string".to_string(),
            "array" => {
                if let Token::Array(array) = token {
                    format!("{}[]", UnityPlugin::map_type(&array.inner))
                } else {
                    panic!("Invalid array token: {:?}", token);
                }
            }
            "tuple" => {
                if let Token::Tuple(tuple) = token {
                    let inners = tuple
                        .inners
                        .iter()
                        .map(UnityPlugin::map_type)
                        .collect::<Vec<String>>()
                        .join(", ");
                    format!("({})", inners)
                } else {
                    panic!("Invalid tuple token: {:?}", token);
                }
            }
            "generic_arg" => {
                if let Token::GenericArg(g) = &token {
                    g.clone()
                } else {
                    panic!("Invalid generic arg token: {:?}", token);
                }
            }

            _ => {
                let mut type_name = token.type_name().to_string();

                if let Token::Composite(composite) = token {
                    if !composite.generic_args.is_empty() {
                        type_name += &format!(
                            "<{}>",
                            composite
                                .generic_args
                                .iter()
                                .map(|(_, t)| UnityPlugin::map_type(t))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                }

                type_name
            }
        }
    }

    fn generated_header() -> String {
        format!(
            "// Generated by dojo-bindgen on {}. Do not modify this file manually.\n",
            chrono::Utc::now().to_rfc2822()
        )
    }

    fn contract_imports() -> String {
        "using System;
using System.Threading.Tasks;
using Dojo;
using Dojo.Starknet;
using UnityEngine;
using dojo_bindings;
using System.Collections.Generic;
using System.Linq;
using Enum = Dojo.Starknet.Enum;
"
        .to_string()
    }

    fn model_imports() -> String {
        "using System;
using Dojo;
using Dojo.Starknet;
using System.Reflection;
using System.Linq;
using System.Collections.Generic;
using Enum = Dojo.Starknet.Enum;
"
        .to_string()
    }

    // Token should be a struct
    // This will be formatted into a C# struct
    // using C# and unity SDK types
    fn format_struct(token: &Composite) -> String {
        let fields = token
            .inners
            .iter()
            .map(|field| format!("public {} {};", UnityPlugin::map_type(&field.token), field.name))
            .collect::<Vec<String>>()
            .join("\n    ");

        format!(
            "
// Type definition for `{}` struct
[Serializable]
public struct {} {{
    {}
}}
",
            token.type_path,
            token.type_name(),
            fields
        )
    }

    // Token should be an enum
    // This will be formatted into a C# enum
    // Enum is mapped using index of cairo enum
    fn format_enum(token: &Composite) -> String {
        let name = token.type_name();
        let mut name_with_generics = name.clone();
        if !token.generic_args.is_empty() {
            name_with_generics += &format!(
                "<{}>",
                token.generic_args.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>().join(", ")
            );
        }

        let mut result = format!(
            "
// Type definition for `{}` enum
public abstract record {}() : Enum {{",
            token.type_path, name_with_generics
        );

        for field in &token.inners {
            let type_name = UnityPlugin::map_type(&field.token).replace(['(', ')'], "");

            result += format!(
                "\n    public record {}({}) : {name_with_generics};",
                field.name,
                if type_name.is_empty() { type_name } else { format!("{} value", type_name) }
            )
            .as_str();
        }

        result += "\n}\n";

        result
    }

    // Token should be a model
    // This will be formatted into a C# class inheriting from ModelInstance
    // Fields are mapped using C# and unity SDK types
    fn format_model(model: &Composite) -> String {
        let fields = model
            .inners
            .iter()
            .map(|field| {
                format!(
                    "[ModelField(\"{}\")]\n    public {} {};",
                    field.name,
                    UnityPlugin::map_type(&field.token),
                    field.name,
                )
            })
            .collect::<Vec<String>>()
            .join("\n\n    ");

        format!(
            "
// Model definition for `{}` model
public class {} : ModelInstance {{
    {}

    // Start is called before the first frame update
    void Start() {{
    }}

    // Update is called once per frame
    void Update() {{
    }}
}}
        ",
            model.type_path,
            model.type_name(),
            fields
        )
    }

    // Handles a model definition and its referenced tokens
    // Will map all structs and enums to C# types
    // Will format the model into a C# class
    fn handle_model(
        &self,
        model: &DojoModel,
        handled_tokens: &mut HashMap<String, Composite>,
    ) -> String {
        let mut out = String::new();
        out += UnityPlugin::generated_header().as_str();
        out += UnityPlugin::model_imports().as_str();

        let mut model_struct: Option<&Composite> = None;
        let tokens = &model.tokens;
        for token in &tokens.structs {
            if handled_tokens.contains_key(&token.type_path()) {
                continue;
            }

            handled_tokens.insert(token.type_path(), token.to_composite().unwrap().to_owned());

            // first index is our model struct
            if token.type_name() == model.name {
                model_struct = Some(token.to_composite().unwrap());
                continue;
            }

            out += UnityPlugin::format_struct(token.to_composite().unwrap()).as_str();
        }

        for token in &tokens.enums {
            if handled_tokens.contains_key(&token.type_path()) {
                continue;
            }

            handled_tokens.insert(token.type_path(), token.to_composite().unwrap().to_owned());
            out += UnityPlugin::format_enum(token.to_composite().unwrap()).as_str();
        }

        out += "\n";

        out += UnityPlugin::format_model(model_struct.expect("model struct not found")).as_str();

        out
    }

    // Formats a system into a C# method used by the contract class
    // Handled tokens should be a list of all structs and enums used by the contract
    // Such as a set of referenced tokens from a model
    fn format_system(system: &Function, handled_tokens: &HashMap<String, Composite>) -> String {
        fn handle_arg_recursive(
            arg_name: &str,
            token: &Token,
            handled_tokens: &HashMap<String, Composite>,
            // variant name
            // if its an enum variant data
            enum_variant: Option<String>,
        ) -> Vec<(
            // formatted arg
            String,
            // if its an array
            bool,
            // enum name and variant name
            // if its an enum variant data
            Option<String>,
        )> {
            let mapped_type = UnityPlugin::map_type(token);

            match token {
                Token::Composite(t) => {
                    let t = handled_tokens.get(&t.type_path).unwrap_or(t);

                    // Need to flatten the struct members.
                    match t.r#type {
                        CompositeType::Struct if t.type_name() == "ByteArray" => vec![(
                            format!("ByteArray.Serialize({}).Select(f => f.Inner)", arg_name),
                            true,
                            enum_variant,
                        )],
                        CompositeType::Struct => {
                            let mut tokens = vec![];
                            t.inners.iter().for_each(|f| {
                                tokens.extend(handle_arg_recursive(
                                    &format!("{}.{}", arg_name, f.name),
                                    &f.token,
                                    handled_tokens,
                                    enum_variant.clone(),
                                ));
                            });

                            tokens
                        }
                        CompositeType::Enum => {
                            let mut tokens = vec![(
                                format!("new FieldElement(Enum.GetIndex({})).Inner", arg_name),
                                false,
                                enum_variant,
                            )];

                            t.inners.iter().for_each(|field| {
                                if let Token::CoreBasic(basic) = &field.token {
                                    // ignore unit type
                                    if basic.type_path == "()" {
                                        return;
                                    }
                                }

                                tokens.extend(handle_arg_recursive(
                                    &format!(
                                        "(({}.{}){}).value",
                                        mapped_type,
                                        field.name.clone(),
                                        arg_name
                                    ),
                                    &if let Token::GenericArg(generic_arg) = &field.token {
                                        let generic_token = t
                                            .generic_args
                                            .iter()
                                            .find(|(name, _)| name == generic_arg)
                                            .unwrap()
                                            .1
                                            .clone();
                                        generic_token
                                    } else {
                                        field.token.clone()
                                    },
                                    handled_tokens,
                                    Some(field.name.clone()),
                                ))
                            });

                            tokens
                        }
                        CompositeType::Unknown => panic!("Unknown composite type: {:?}", t),
                    }
                }
                Token::Array(array) => {
                    let is_inner_array = matches!(array.inner.as_ref(), Token::Array(_));
                    let inner = handle_arg_recursive(
                        &format!("{arg_name}Item"),
                        &array.inner,
                        handled_tokens,
                        enum_variant.clone(),
                    );

                    let inners = inner
                        .into_iter()
                        .map(|(arg, _, _)| arg)
                        .collect::<Vec<String>>()
                        .join(", ");

                    vec![
                        (
                            format!("new FieldElement({arg_name}.Length).Inner",),
                            false,
                            enum_variant.clone(),
                        ),
                        (
                            if is_inner_array {
                                format!(
                                    "{arg_name}.SelectMany({arg_name}Item => new \
                                     dojo.FieldElement[] {{ }}.Concat({inners}))"
                                )
                            } else {
                                format!(
                                    "{arg_name}.SelectMany({arg_name}Item => new [] {{ {inners} \
                                     }})"
                                )
                            },
                            true,
                            enum_variant.clone(),
                        ),
                    ]
                }
                Token::Tuple(tuple) => tuple
                    .inners
                    .iter()
                    .enumerate()
                    .flat_map(|(idx, token)| {
                        handle_arg_recursive(
                            &format!("{}.Item{}", arg_name, idx + 1),
                            token,
                            handled_tokens,
                            enum_variant.clone(),
                        )
                    })
                    .collect(),
                _ => match mapped_type.as_str() {
                    "FieldElement" => vec![(format!("{}.Inner", arg_name), false, enum_variant)],
                    _ => {
                        vec![(format!("new FieldElement({}).Inner", arg_name), false, enum_variant)]
                    }
                },
            }
        }

        let args = system
            .inputs
            .iter()
            .map(|arg| format!("{} {}", UnityPlugin::map_type(&arg.1), &arg.0))
            .collect::<Vec<String>>()
            .join(", ");

        let calldata = system
            .inputs
            .iter()
            .flat_map(|(name, token)| {
                let tokens = handle_arg_recursive(name, token, handled_tokens, None);

                tokens
                    .iter()
                    .map(|(arg, is_array, enum_variant)| {
                        let calldata_op = if *is_array {
                            format!("calldata.AddRange({arg});")
                        } else {
                            format!("calldata.Add({arg});")
                        };

                        if let Some(variant) = enum_variant {
                            let mapped_token = UnityPlugin::map_type(token);
                            let mapped_variant_type = format!("{}.{}", mapped_token, variant);

                            format!("if ({name} is {mapped_variant_type}) {calldata_op}",)
                        } else {
                            calldata_op
                        }
                    })
                    .collect::<Vec<String>>()
            })
            .collect::<Vec<String>>()
            .join("\n\t\t");

        format!(
            "
    // Call the `{system_name}` system with the specified Account and calldata
    // Returns the transaction hash. Use `WaitForTransaction` to wait for the transaction to be \
             confirmed.
    public async Task<FieldElement> {system_name}(Account account{arg_sep}{args}) {{
        List<dojo.FieldElement> calldata = new List<dojo.FieldElement>();
        {calldata}

        return await account.ExecuteRaw(new dojo.Call[] {{
            new dojo.Call{{
                to = contractAddress,
                selector = \"{system_name}\",
                calldata = calldata.ToArray()
            }}
        }});
    }}
            ",
            // selector for execute
            system_name = system.name,
            // add comma if we have args
            arg_sep = if !args.is_empty() { ", " } else { "" },
            // formatted args to use our mapped types
            args = args,
            // calldata for execute
            calldata = calldata
        )
    }

    // Formats a contract file path into a pretty contract name
    // eg. dojo_examples::actions::actions.json -> Actions
    fn formatted_contract_name(contract_file_name: &str) -> String {
        let contract_name =
            contract_file_name.split("::").last().unwrap().trim_end_matches(".json");
        // capitalize contract name
        contract_name.chars().next().unwrap().to_uppercase().to_string() + &contract_name[1..]
    }

    // Handles a contract definition and its underlying systems
    // Will format the contract into a C# class and
    // all systems into C# methods
    // Handled tokens should be a list of all structs and enums used by the contract
    fn handle_contract(
        &self,
        contract: &DojoContract,
        handled_tokens: &HashMap<String, Composite>,
    ) -> String {
        let mut out = String::new();
        out += UnityPlugin::generated_header().as_str();
        out += UnityPlugin::contract_imports().as_str();

        let systems = contract
            .systems
            .iter()
            // we assume systems dont have outputs
            .filter(|s| s.to_function().unwrap().get_output_kind() as u8 == FunctionOutputKind::NoOutput as u8)
            .map(|system| UnityPlugin::format_system(system.to_function().unwrap(), handled_tokens))
            .collect::<Vec<String>>()
            .join("\n\n    ");

        out += &format!(
            "
// System definitions for `{}` contract
public class {} : MonoBehaviour {{
    // The address of this contract
    public string contractAddress;

    {}
}}
        ",
            contract.qualified_path,
            // capitalize contract name
            UnityPlugin::formatted_contract_name(&contract.qualified_path),
            systems
        );

        out
    }
}

#[async_trait]
impl BuiltinPlugin for UnityPlugin {
    async fn generate_code(&self, data: &DojoData) -> BindgenResult<HashMap<PathBuf, Vec<u8>>> {
        let mut out: HashMap<PathBuf, Vec<u8>> = HashMap::new();
        let mut handled_tokens = HashMap::<String, Composite>::new();

        // Handle codegen for models
        for (name, model) in &data.models {
            let models_path = Path::new(&format!("Models/{}.gen.cs", name)).to_owned();

            println!("Generating model: {}", name);
            let code = self.handle_model(model, &mut handled_tokens);

            out.insert(models_path, code.as_bytes().to_vec());
        }

        // Handle codegen for systems
        for (name, contract) in &data.contracts {
            let contracts_path = Path::new(&format!("Contracts/{}.gen.cs", name)).to_owned();

            println!("Generating contract: {}", name);
            let code = self.handle_contract(contract, &handled_tokens);

            out.insert(contracts_path, code.as_bytes().to_vec());
        }

        Ok(out)
    }
}