use std::collections::HashMap;
use std::fs;

use cainome::parser::tokens::Token;
use cainome::parser::AbiParser;
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
    pub name: String,
    pub qualified_path: String,
    pub tokens: HashMap<String, Vec<Token>>,
}

#[derive(Debug, PartialEq)]
pub struct DojoContract {
    pub contract_file_name: String,
    pub tokens: HashMap<String, Vec<Token>>,
}

#[derive(Debug)]
pub struct DojoData {
    pub contracts: HashMap<String, DojoContract>,
    pub models: HashMap<String, DojoModel>,
}

// TODO: include the manifest to have more metadata when new manifest is available.
#[derive(Debug)]
pub struct PluginManager {
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

            builder.generate_code(&data).await?;
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
                        contracts.insert(
                            file_name.to_string(),
                            DojoContract {
                                contract_file_name: file_name.to_string(),
                                tokens: tokens.clone(),
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
fn filter_model_tokens(tokens: &HashMap<String, Vec<Token>>) -> HashMap<String, Vec<Token>> {
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

    for s in tokens.get("structs").unwrap() {
        if !skip_token(s) {
            structs.push(s.clone());
        }
    }

    for e in tokens.get("enums").unwrap() {
        if !skip_token(e) {
            enums.push(e.clone());
        }
    }

    let mut model_tokens = HashMap::new();
    // All functions can be ignored, the client does not need to call them directly.
    model_tokens.insert(String::from("structs"), structs);
    model_tokens.insert(String::from("enums"), enums);
    model_tokens.insert(String::from("functions"), vec![]);

    model_tokens
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
fn is_model_contract(tokens: &HashMap<String, Vec<Token>>) -> bool {
    let expected_funcs = ["name", "layout", "packed_size", "unpacked_size", "schema"];

    let mut funcs_counts = 0;

    // This hashmap is not that good at devex level.. one must check the
    // code to know the keys.
    for f in &tokens["functions"] {
        if expected_funcs.contains(&f.to_function().expect("Function expected").name.as_str()) {
            funcs_counts += 1;
        }
    }

    funcs_counts == expected_funcs.len()
}

// Spawn and move project is not built at the time this lib is being tested.
// Need plain artifacts to simplify or use `sozo` from env (available as we're in the devcontainer).
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
//     fn gather_models_ok() {
//         let models =
//
// gather_models(&Utf8PathBuf::from("src/test_data/spawn-and-move/target/dev")).unwrap();

//         assert_eq!(models.len(), 2);

//         let pos = models.get("Position").unwrap();
//         assert_eq!(pos.name, "Position");
//         assert_eq!(pos.qualified_path, "dojo_examples::models::Position");

//         let moves = models.get("Moves").unwrap();
//         assert_eq!(moves.name, "Moves");
//         assert_eq!(moves.qualified_path, "dojo_examples::models::Moves");
//     }
// }
