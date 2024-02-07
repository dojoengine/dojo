use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use cainome::parser::tokens::Token;
use cainome::parser::{AbiParser, TokenizedAbi};
use camino::Utf8PathBuf;
use convert_case::{Case, Casing};

pub mod error;
use error::{BindgenResult, Error};

mod plugins;
use plugins::typescript::TypescriptPlugin;
use plugins::unity::UnityPlugin;
use plugins::BuiltinPlugin;
pub use plugins::BuiltinPlugins;

#[derive(Debug, PartialEq)]
pub struct DojoModel {
    /// PascalCase name of the model.
    pub name: String,
    /// Fully qualified path of the model type in cairo code.
    pub qualified_path: String,
    /// List of tokens found in the model contract ABI.
    /// Only structs and enums are currently used.
    pub tokens: TokenizedAbi,
}

#[derive(Debug, PartialEq)]
pub struct DojoContract {
    /// Contract's name.
    pub contract_file_name: String,
    /// Full ABI of the contract in case the plugin wants to make extra checks,
    /// or generated other functions than the systems.
    pub tokens: TokenizedAbi,
    /// Functions that are identified as systems.
    pub systems: Vec<Token>,
}

#[derive(Debug)]
pub struct DojoData {
    /// All contracts found in the project.
    pub contracts: HashMap<String, DojoContract>,
    /// All the models contracts found in the project.
    pub models: HashMap<String, DojoModel>,
}

// TODO: include the manifest to have more metadata when new manifest is available.
#[derive(Debug)]
pub struct PluginManager {
    /// Path of generated files.
    pub output_path: PathBuf,
    /// Path of contracts artifacts.
    pub artifacts_path: Utf8PathBuf,
    /// A list of builtin plugins to invoke.
    pub builtin_plugins: Vec<BuiltinPlugins>,
    /// A list of custom plugins to invoke.
    pub plugins: Vec<String>,
}

impl PluginManager {
    /// Generates the bindings for all the given Plugin.
    pub async fn generate(&self) -> BindgenResult<()> {
        if self.builtin_plugins.is_empty() && self.plugins.is_empty() {
            return Ok(());
        }

        let data = gather_dojo_data(&self.artifacts_path)?;

        for plugin in &self.builtin_plugins {
            // Get the plugin builder from the plugin enum.
            let builder: Box<dyn BuiltinPlugin> = match plugin {
                BuiltinPlugins::Typescript => Box::new(TypescriptPlugin::new()),
                BuiltinPlugins::Unity => Box::new(UnityPlugin::new()),
            };

            let files = builder.generate_code(&data).await?;
            for (path, content) in files {
                // Prepends the output directory and plugin name to the path.
                let path = self.output_path.join(plugin.to_string()).join(path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();

                fs::write(path, content)?;
            }
        }
        Ok(())
    }
}

/// Gathers dojo data from artifacts.
/// TODO: this should be modified later to use the new manifest structure.
///       it's currently done from the artifacts to decouple from the manifest.
///
/// # Arguments
///
/// * `artifacts_path` - Artifacts path where contracts were generated.
fn gather_dojo_data(artifacts_path: &Utf8PathBuf) -> BindgenResult<DojoData> {
    let mut models = HashMap::new();
    let mut contracts = HashMap::new();

    for entry in fs::read_dir(artifacts_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                let file_content = fs::read_to_string(&path)?;

                // Models and Contracts must have a valid ABI.
                if let Ok(tokens) =
                    AbiParser::tokens_from_abi_string(&file_content, &HashMap::new())
                {
                    // Contract.
                    if is_systems_contract(file_name, &file_content) {
                        // Identify the systems -> for now only take the functions from the
                        // interfaces.
                        let mut systems = vec![];
                        let interface_blacklist = [
                            "dojo::world::IWorldProvider",
                            "dojo::components::upgradeable::IUpgradeable",
                        ];

                        for (interface, funcs) in &tokens.interfaces {
                            if !interface_blacklist.contains(&interface.as_str()) {
                                systems.extend(funcs.clone());
                            }
                        }

                        contracts.insert(
                            file_name.to_string(),
                            DojoContract {
                                contract_file_name: file_name.to_string(),
                                tokens: tokens.clone(),
                                systems,
                            },
                        );
                    }

                    // Model.
                    if is_model_contract(&tokens) {
                        if let Some(model_name) = model_name_from_artifact_filename(file_name) {
                            let model_pascal_case =
                                model_name.from_case(Case::Snake).to_case(Case::Pascal);

                            let model = DojoModel {
                                name: model_pascal_case.clone(),
                                qualified_path: file_name
                                    .replace(&model_name, &model_pascal_case)
                                    .trim_end_matches(".json")
                                    .to_string(),
                                tokens: filter_model_tokens(&tokens),
                            };

                            models.insert(model_pascal_case, model);
                        } else {
                            return Err(Error::Format(format!(
                                "Could not extract model name from file name `{file_name}`"
                            )));
                        }
                    }
                }
            }
        }
    }

    Ok(DojoData { models, contracts })
}

/// Identifies if the given contract contains systems.
///
/// For now the identification is very naive and don't use the manifest
/// as the manifest format will change soon.
/// TODO: use the new manifest files once available.
///
/// # Arguments
///
/// * `file_name` - Name of the contract file.
/// * `file_content` - Content of the contract artifact.
fn is_systems_contract(file_name: &str, file_content: &str) -> bool {
    if file_name.starts_with("dojo::") || file_name == "manifest.json" {
        return false;
    }

    file_content.contains("IWorldDispatcher")
}

/// Filters the model ABI to keep relevant types
/// to be generated for bindings.
fn filter_model_tokens(tokens: &TokenizedAbi) -> TokenizedAbi {
    let mut structs = vec![];
    let mut enums = vec![];

    // All types from introspect module can also be removed as the clients does not rely on them.
    // Events are also always empty at model contract level.
    fn skip_token(token: &Token) -> bool {
        if token.type_path().starts_with("dojo::database::introspect") {
            return true;
        }

        if let Token::Composite(c) = token {
            if c.is_event {
                return true;
            }
        }

        false
    }

    for s in &tokens.structs {
        if !skip_token(s) {
            structs.push(s.clone());
        }
    }

    for e in &tokens.enums {
        if !skip_token(e) {
            enums.push(e.clone());
        }
    }

    TokenizedAbi { structs, enums, ..Default::default() }
}

/// Extracts a model name from the artifact file name.
///
/// # Example
///
/// The file name "dojo_examples::models::position.json" should return "position".
///
/// # Arguments
///
/// * `file_name` - Artifact file name.
fn model_name_from_artifact_filename(file_name: &str) -> Option<String> {
    let parts: Vec<&str> = file_name.split("::").collect();

    if let Some(last_part) = parts.last() {
        // TODO: for now, we always reconstruct with PascalCase.
        // Once manifest data are available, use the exact name instead.
        // We may have errors here is the struct is named like myStruct and not MyStruct.
        // Plugin dev should consider case insensitive comparison.
        last_part.split_once(".json").map(|m_ext| m_ext.0.to_string())
    } else {
        None
    }
}

/// Identifies if the given contract contains a model.
///
/// The identification is based on the methods name. This must
/// be adjusted if the model attribute expansion change in the future.
/// <https://github.com/dojoengine/dojo/blob/36e5853877d011a5bb4b3bd77b9de676fb454b0c/crates/dojo-lang/src/model.rs#L81>
///
/// # Arguments
///
/// * `file_name` - Name of the contract file.
/// * `file_content` - Content of the contract artifact.
fn is_model_contract(tokens: &TokenizedAbi) -> bool {
    let expected_funcs = ["name", "layout", "packed_size", "unpacked_size", "schema"];

    let mut funcs_counts = 0;

    for functions in tokens.interfaces.values() {
        for f in functions {
            if expected_funcs.contains(&f.to_function().expect("Function expected").name.as_str()) {
                funcs_counts += 1;
            }
        }
    }

    funcs_counts == expected_funcs.len()
}

// Uncomment tests once windows issue is solved.
// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn is_system_contract_ok() {
//         let file_name = "dojo_examples::actions::actions.json";
//         let file_content = include_str!(
//             "test_data/spawn-and-move/target/dev/dojo_examples::actions::actions.json"
//         );

//         assert!(is_systems_contract(file_name, file_content));
//     }

//     #[test]
//     fn is_system_contract_ignore_dojo_files() {
//         let file_name = "dojo::world::world.json";
//         let file_content = "";
//         assert!(!is_systems_contract(file_name, file_content));

//         let file_name = "manifest.json";
//         assert!(!is_systems_contract(file_name, file_content));
//     }

//     #[test]
//     fn test_is_system_contract_ignore_models() {
//         let file_name = "dojo_examples::models::position.json";
//         let file_content = include_str!(
//             "test_data/spawn-and-move/target/dev/dojo_examples::models::position.json"
//         );
//         assert!(!is_systems_contract(file_name, file_content));
//     }

//     #[test]
//     fn model_name_from_artifact_filename_ok() {
//         let file_name = "dojo_examples::models::position.json";
//         assert_eq!(model_name_from_artifact_filename(file_name), Some("position".to_string()));
//     }

//     #[test]
//     fn is_model_contract_ok() {
//         let file_content =
//
// include_str!("test_data/spawn-and-move/target/dev/dojo_examples::models::moves.json");
//         let tokens = AbiParser::tokens_from_abi_string(file_content, &HashMap::new()).unwrap();

//         assert!(is_model_contract(&tokens));
//     }

//     #[test]
//     fn is_model_contract_ignore_systems() {
//         let file_content = include_str!(
//             "test_data/spawn-and-move/target/dev/dojo_examples::actions::actions.json"
//         );
//         let tokens = AbiParser::tokens_from_abi_string(file_content, &HashMap::new()).unwrap();

//         assert!(!is_model_contract(&tokens));
//     }

//     #[test]
//     fn is_model_contract_ignore_dojo_files() {
//         let file_content =
//             include_str!("test_data/spawn-and-move/target/dev/dojo::world::world.json");
//         let tokens = AbiParser::tokens_from_abi_string(file_content, &HashMap::new()).unwrap();

//         assert!(!is_model_contract(&tokens));
//     }

//     #[test]
//     fn gather_data_ok() {
//         let data =
// gather_dojo_data(&Utf8PathBuf::from("src/test_data/spawn-and-move/target/dev"))
// .unwrap();

//         assert_eq!(data.models.len(), 2);

//         let pos = data.models.get("Position").unwrap();
//         assert_eq!(pos.name, "Position");
//         assert_eq!(pos.qualified_path, "dojo_examples::models::Position");

//         let moves = data.models.get("Moves").unwrap();
//         assert_eq!(moves.name, "Moves");
//         assert_eq!(moves.qualified_path, "dojo_examples::models::Moves");
//     }
// }
