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
}

#[derive(Debug)]
pub struct DojoMetadata {
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

        println!("Generating bindings {:?}", self);

        // TODO: loops can be optimized to only parse a file once.
        let metadata = DojoMetadata {
            models: gather_models(&self.artifacts_path).expect("Can't gather models"),
        };

        for plugin in &self.builtin_plugins {
            // Get the plugin builder from the plugin enum.
            let builder: Box<dyn BuiltinPlugin> = match plugin {
                BuiltinPlugins::Typescript => Box::new(TypescriptPlugin::new()),
                BuiltinPlugins::Unity => Box::new(UnityPlugin::new()),
            };

            // TODO: types aliases: For now they are empty, we can expect them to be passed
            // by the user from command line. But in dojo context, the naming conflict
            // in a contract are low as they remain usually relatively simple.
            let types_aliases = HashMap::new();

            for entry in fs::read_dir(&self.artifacts_path)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        let file_content = fs::read_to_string(&path)?;

                        if is_systems_contract(file_name, &file_content) {
                            let tokens =
                                AbiParser::tokens_from_abi_string(&file_content, &types_aliases)?;
                            builder.generate_systems_bindings(file_name, tokens, &metadata).await?;
                        }
                    }
                }
            }
        }

        // TODO: invoke the custom plugins via stdin.
        // TODO: define the interface to pass the data to the plugin. JSON? Protobuf?
        // (cf. mod.rs in plugins module).
        // The plugin executable (same name as the plugin name) MUST be in the path.

        Ok(())
    }
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

/// Gathers the models from given artifacts path.
///
/// This may be done using the manifest when new manifest structure
/// is defined and implemented.
///
/// # Arguments
///
/// * `artifacts_path` - Artifacts path where model contracts were generated.
fn gather_models(artifacts_path: &Utf8PathBuf) -> BindgenResult<HashMap<String, DojoModel>> {
    println!("ARTIF PATH: {}", artifacts_path);
    let mut models = HashMap::new();

    for entry in fs::read_dir(artifacts_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                let file_content = fs::read_to_string(&path)?;

                if let Ok(tokens) =
                    AbiParser::tokens_from_abi_string(&file_content, &HashMap::new())
                {
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

    Ok(models)
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
    // TODO: change for an enum instead of string.
    for f in &tokens["functions"] {
        if expected_funcs.contains(&f.to_function().expect("Function expected").name.as_str()) {
            funcs_counts += 1;
        }
    }

    funcs_counts == expected_funcs.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_system_contract_ok() {
        let file_name = "dojo_examples::actions::actions.json";
        let file_content = include_str!(
            "./test_data/spawn-and-move/target/dev/dojo_examples::actions::actions.json"
        );

        assert!(is_systems_contract(file_name, file_content));
    }

    #[test]
    fn is_system_contract_ignore_dojo_files() {
        let file_name = "dojo::world::world.json";
        let file_content = "";
        assert!(!is_systems_contract(file_name, file_content));

        let file_name = "manifest.json";
        assert!(!is_systems_contract(file_name, file_content));
    }

    #[test]
    fn test_is_system_contract_ignore_models() {
        let file_name = "dojo_examples::models::position.json";
        let file_content = include_str!(
            "./test_data/spawn-and-move/target/dev/dojo_examples::models::position.json"
        );
        assert!(!is_systems_contract(file_name, file_content));
    }

    #[test]
    fn model_name_from_artifact_filename_ok() {
        let file_name = "dojo_examples::models::position.json";
        assert_eq!(model_name_from_artifact_filename(file_name), Some("position".to_string()));
    }

    #[test]
    fn is_model_contract_ok() {
        let file_content =
            include_str!("./test_data/spawn-and-move/target/dev/dojo_examples::models::moves.json");
        let tokens = AbiParser::tokens_from_abi_string(file_content, &HashMap::new()).unwrap();

        assert!(is_model_contract(&tokens));
    }

    #[test]
    fn is_model_contract_ignore_systems() {
        let file_content = include_str!(
            "./test_data/spawn-and-move/target/dev/dojo_examples::actions::actions.json"
        );
        let tokens = AbiParser::tokens_from_abi_string(file_content, &HashMap::new()).unwrap();

        assert!(!is_model_contract(&tokens));
    }

    #[test]
    fn is_model_contract_ignore_dojo_files() {
        let file_content =
            include_str!("./test_data/spawn-and-move/target/dev/dojo::world::world.json");
        let tokens = AbiParser::tokens_from_abi_string(file_content, &HashMap::new()).unwrap();

        assert!(!is_model_contract(&tokens));
    }

    #[test]
    fn gather_models_ok() {
        let models =
            gather_models(&Utf8PathBuf::from("./src/test_data/spawn-and-move/target/dev")).unwrap();

        assert_eq!(models.len(), 2);
        assert_eq!(
            models.get("Position").unwrap(),
            &DojoModel {
                name: "Position".to_string(),
                qualified_path: "dojo_examples::models::Position".to_string()
            }
        );
        assert_eq!(
            models.get("Moves").unwrap(),
            &DojoModel {
                name: "Moves".to_string(),
                qualified_path: "dojo_examples::models::Moves".to_string()
            }
        );
    }
}
