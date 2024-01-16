use std::any::Any;

use async_trait::async_trait;
use cainome::parser::tokens::{Composite, CompositeType, Function, Token};

use crate::error::{BindgenResult, Error};
use crate::plugins::BuiltinPlugin;
use crate::{DojoContract, DojoData, DojoModel};

#[derive(Debug)]
pub enum UnityPluginError {
    InvalidType(String),
}

impl std::fmt::Display for UnityPluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            UnityPluginError::InvalidType(type_path) => write!(f, "Invalid type: {}", type_path),
        }
    }
}

impl std::error::Error for UnityPluginError {}

pub struct UnityPlugin {}

impl UnityPlugin {
    pub fn new() -> Self {
        Self {}
    }

    // Maps cairo types to C#/Unity SDK defined types
    fn map_type(type_name: &str) -> Result<String, UnityPluginError> {
        match type_name {
            "u8" => Ok("byte".to_string()),
            "u16" => Ok("ushort".to_string()),
            "u32" => Ok("uint".to_string()),
            "u64" => Ok("ulong".to_string()),
            "u128" => Ok("BigInteger".to_string()),
            "u256" => Ok("BigInteger".to_string()),
            "usize" => Ok("uint".to_string()),
            "felt252" => Ok("FieldElement".to_string()),
            "ClassHash" => Ok("FieldElement".to_string()),
            "ContractAddress" => Ok("FieldElement".to_string()),

            _ => Ok(type_name.to_string()),
        }
    }

    // Token should be a struct
    // This will be formatted into a C# struct
    // using C# and unity SDK types
    fn format_struct(token: &Composite) -> Result<String, UnityPluginError> {
        let fields = token
            .inners
            .iter()
            .map(|field| {
                format!(
                    "public {} {};",
                    UnityPlugin::map_type(field.token.clone().type_name().as_str()).unwrap(),
                    field.name
                )
            })
            .collect::<Vec<String>>()
            .join("\n    ");

        return Ok(format!(
            "
[Serializable]
public struct {} {{
    {}
}}
",
            token.type_name(),
            fields
        ));
    }

    // Token should be an enum
    // This will be formatted into a C# enum
    // Enum is mapped using index of cairo enum
    fn format_enum(token: &Composite) -> Result<String, UnityPluginError> {
        let fields = token
            .inners
            .iter()
            .map(|field| format!("{},", field.name,))
            .collect::<Vec<String>>()
            .join("\n    ");

        return Ok(format!(
            "
public enum {} {{
    {}
}}
",
            token.type_name(),
            fields
        ));
    }

    fn format_model(model: &Composite) -> Result<String, UnityPluginError> {
        let fields = model
            .inners
            .iter()
            .map(|field| {
                format!(
                    "[ModelField(\"{}\")]\n    public {} {};",
                    field.name,
                    UnityPlugin::map_type(field.token.type_name().as_str()).unwrap(),
                    field.name,
                )
            })
            .collect::<Vec<String>>()
            .join("\n\n    ");

        return Ok(format!(
            "
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
            model.type_name(),
            fields
        ));
    }

    fn handle_model(
        &self,
        model: &DojoModel,
        handled_tokens: &mut Vec<Composite>,
    ) -> Result<String, UnityPluginError> {
        let mut out = String::new();
        out += "using System;\n";
        out += "using Dojo;\n";
        out += "using Dojo.Starknet;\n";

        let mut model_struct: Option<&Composite> = None;
        let tokens = &model.tokens;
        for token in &tokens.structs {
            handled_tokens.push(token.to_composite().unwrap().to_owned());

            // first index is our model struct
            if token.type_name() == model.name {
                model_struct = Some(token.to_composite().unwrap());
                continue;
            }

            out += UnityPlugin::format_struct(token.to_composite().unwrap())?.as_str();
        }

        for token in &tokens.enums {
            handled_tokens.push(token.to_composite().unwrap().to_owned());
            out += UnityPlugin::format_enum(token.to_composite().unwrap())?.as_str();
        }

        out += "\n";

        out += UnityPlugin::format_model(model_struct.expect("model struct not found"))?.as_str();

        Ok(out)
    }

    fn format_system(
        system: &Function,
        handled_tokens: &Vec<Composite>,
    ) -> Result<String, UnityPluginError> {
        let args = system
            .inputs
            .iter()
            .map(|arg| format!("{} {}", UnityPlugin::map_type(&arg.1.type_name()).unwrap(), arg.0,))
            .collect::<Vec<String>>()
            .join(", ");

        let calldata = system
            .inputs
            .iter()
            .map(|arg| {
                let token = arg.1.to_composite().unwrap();
                println!("{:?}", token);
                // r#type doesnt seem to be working rn.
                // match token.unwrap().r#type {
                //     CompositeType::Struct => {
                //         // doesnt support structs yet
                //         format!("new FieldElement({})", arg.0)
                //     }
                //     _ => {
                //         format!("new FieldElement({})", arg.0)
                //     }
                // }

                // instead, we can take a look at our
                // handled tokens db
                let token =
                    handled_tokens.iter().find(|t| t.type_name() == token.type_name()).unwrap();

                match token.r#type {
                    CompositeType::Struct => token
                        .inners
                        .iter()
                        .map(|field| format!("new FieldElement({}.{})", arg.0, field.name))
                        .collect::<Vec<String>>()
                        .join(",\n                    "),
                    _ => {
                        format!("new FieldElement({})", arg.0)
                    }
                }
            })
            .collect::<Vec<String>>()
            .join(",\n                ");

        Ok(format!(
            "
    public async Task<FieldElement> {}(Account account{}{}) {{
        return await account.ExecuteRaw(new dojo.Call[] {{
            new dojo.Call{{
                to = contractAddress,
                selector = \"{}\",
                calldata = new FieldElement[] {{
                    {}
                }}
            }}
        }});
    }}
            ",
            // capitalize system name
            system.name.chars().next().unwrap().to_uppercase().to_string() + &system.name[1..],
            // add comma if we have args
            if args.len() > 0 { ", " } else { "" },
            // formatted args to use our mapped types
            args,
            // selector for execute
            system.name,
            // calldata for execute
            calldata
        ))
    }

    fn handle_contract(
        &self,
        contract: &DojoContract,
        handled_tokens: &Vec<Composite>,
    ) -> Result<String, UnityPluginError> {
        let mut out = String::new();
        out += "using System;\n";
        out += "using Dojo;\n";
        out += "using Dojo.Starknet;\n";

        let contract_name =
            contract.contract_file_name.split("::").last().unwrap().trim_end_matches(".json");
        // capitalize contract name
        let contract_name =
            contract_name.chars().next().unwrap().to_uppercase().to_string() + &contract_name[1..];

        let systems = contract
            .systems
            .iter()
            .map(|system| {
                UnityPlugin::format_system(system.to_function().unwrap(), handled_tokens).unwrap()
            })
            .collect::<Vec<String>>()
            .join("\n\n    ");

        out += &format!(
            "
public class {} : MonoBehaviour {{
    public string contractAddress;

    {}
}}
        ",
            // capitalize contract name
            contract_name,
            systems
        );

        Ok(out)
    }
}

#[async_trait]
impl BuiltinPlugin for UnityPlugin {
    async fn generate_code(&self, data: &DojoData) -> BindgenResult<()> {
        let mut handled_tokens = Vec::<Composite>::new();

        // Handle codegen for models
        for (name, model) in &data.models {
            println!("Generating model: {}", name);
            let code = self
                .handle_model(model, &mut handled_tokens)
                .map_err(|e| Error::Format(format!("Failed to generate code for model: {}", e)))?;
            println!("{}", code);
        }

        // Handle codegen for systems
        for (name, contract) in &data.contracts {
            println!("Generating contract: {}", name);
            let code = self.handle_contract(contract, &handled_tokens).map_err(|e| {
                Error::Format(format!("Failed to generate code for contract: {}", e))
            })?;
            println!("{}", code);
        }

        Ok(())
    }
}
