use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use cainome::parser::tokens::Token;
use cainome::parser::{AbiParser, TokenizedAbi};
use camino::Utf8PathBuf;
use convert_case::{Case, Casing};
use dojo_world::manifest::BaseManifest;
pub mod error;
use error::{BindgenResult, Error};

mod plugins;
use plugins::typescript::TypescriptPlugin;
use plugins::typescript_v2::TypeScriptV2Plugin;
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
    /// Contract's fully qualified name.
    pub qualified_path: String,
    /// Full ABI of the contract in case the plugin wants to make extra checks,
    /// or generated other functions than the systems.
    pub tokens: TokenizedAbi,
    /// Functions that are identified as systems.
    pub systems: Vec<Token>,
}

#[derive(Debug, PartialEq)]
pub struct DojoWorld {
    /// The world's name from the Scarb manifest.
    pub name: String,
}

#[derive(Debug)]
pub struct DojoData {
    /// World data.
    pub world: DojoWorld,
    /// All contracts found in the project.
    pub contracts: HashMap<String, DojoContract>,
    /// All the models contracts found in the project.
    pub models: HashMap<String, DojoModel>,
}

#[derive(Debug)]
pub struct PluginManager {
    /// Profile name.
    pub profile_name: String,
    /// Root package name.
    pub root_package_name: String,
    /// Path of generated files.
    pub output_path: PathBuf,
    /// Path of Dojo manifest.
    pub manifest_path: Utf8PathBuf,
    /// A list of builtin plugins to invoke.
    pub builtin_plugins: Vec<BuiltinPlugins>,
    /// A list of custom plugins to invoke.
    pub plugins: Vec<String>,
}

impl PluginManager {
    /// Generates the bindings for all the given Plugin.
    pub async fn generate(&self, skip_migration: Option<Vec<String>>) -> BindgenResult<()> {
        if self.builtin_plugins.is_empty() && self.plugins.is_empty() {
            return Ok(());
        }

        let data = gather_dojo_data(
            &self.manifest_path,
            &self.root_package_name,
            &self.profile_name,
            skip_migration,
        )?;

        for plugin in &self.builtin_plugins {
            // Get the plugin builder from the plugin enum.
            let builder: Box<dyn BuiltinPlugin> = match plugin {
                BuiltinPlugins::Typescript => Box::new(TypescriptPlugin::new()),
                BuiltinPlugins::Unity => Box::new(UnityPlugin::new()),
                BuiltinPlugins::TypeScriptV2 => Box::new(TypeScriptV2Plugin::new()),
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

/// Gathers dojo data from the manifests files.
///
/// # Arguments
///
/// * `manifest_path` - Dojo manifest path.
fn gather_dojo_data(
    manifest_path: &Utf8PathBuf,
    root_package_name: &str,
    profile_name: &str,
    skip_migration: Option<Vec<String>>,
) -> BindgenResult<DojoData> {
    let root_dir: Utf8PathBuf = manifest_path.parent().unwrap().into();
    let base_manifest_dir: Utf8PathBuf = root_dir.join("manifests").join(profile_name).join("base");
    let mut base_manifest = BaseManifest::load_from_path(&base_manifest_dir)?;

    if let Some(skip_manifests) = skip_migration {
        base_manifest.remove_items(skip_manifests);
    }

    let mut models = HashMap::new();
    let mut contracts = HashMap::new();

    for contract_manifest in &base_manifest.contracts {
        // Base manifest always use path for ABI.
        let abi = contract_manifest
            .inner
            .abi
            .as_ref()
            .expect("Valid ABI for contract")
            .load_abi_string(&root_dir)?;

        let tokens = AbiParser::tokens_from_abi_string(&abi, &HashMap::new())?;

        // Identify the systems -> for now only take the functions from the
        // interfaces.
        let mut systems = vec![];
        let interface_blacklist =
            ["dojo::world::IWorldProvider", "dojo::components::upgradeable::IUpgradeable"];

        for (interface, funcs) in &tokens.interfaces {
            if !interface_blacklist.contains(&interface.as_str()) {
                systems.extend(funcs.clone());
            }
        }

        let contract_name = contract_manifest.name.to_string();

        contracts.insert(
            contract_name.clone(),
            DojoContract { qualified_path: contract_name, tokens, systems },
        );
    }

    for model_manifest in &base_manifest.models {
        // Base manifest always use path for ABI.
        let abi = model_manifest
            .inner
            .abi
            .as_ref()
            .expect("Valid ABI for contract")
            .load_abi_string(&root_dir)?;

        let tokens = AbiParser::tokens_from_abi_string(&abi, &HashMap::new())?;

        let name = model_manifest.name.to_string();

        if let Some(model_name) = model_name_from_fully_qualified_path(&name) {
            let model_pascal_case = model_name.from_case(Case::Snake).to_case(Case::Pascal);

            let model = DojoModel {
                name: model_pascal_case.clone(),
                qualified_path: name
                    .replace(&model_name, &model_pascal_case)
                    .trim_end_matches(".json")
                    .to_string(),
                tokens: filter_model_tokens(&tokens),
            };

            models.insert(model_pascal_case, model);
        } else {
            return Err(Error::Format(format!(
                "Could not extract model name from file name `{name}`"
            )));
        }
    }

    let world = DojoWorld { name: root_package_name.to_string() };

    Ok(DojoData { world, models, contracts })
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

/// Extracts a model name from the fully qualified path of the model.
///
/// # Example
///
/// The fully qualified name "dojo_examples::models::position" should return "position".
///
/// # Arguments
///
/// * `file_name` - Fully qualified model name.
fn model_name_from_fully_qualified_path(file_name: &str) -> Option<String> {
    let parts: Vec<&str> = file_name.split("::").collect();

    // TODO: we may want to have inside the manifest the name of the model struct
    // instead of extracting it from the file's name.
    parts.last().map(|last_part| last_part.to_string())
}

#[cfg(test)]
mod tests {
    use dojo_test_utils::compiler;
    use dojo_world::metadata::dojo_metadata_from_workspace;

    use super::*;

    #[test]
    fn model_name_from_fully_qualified_path_ok() {
        let file_name = "dojo_examples::models::position";
        assert_eq!(model_name_from_fully_qualified_path(file_name), Some("position".to_string()));
    }

    #[test]
    fn gather_data_ok() {
        let manifest_path = Utf8PathBuf::from("src/test_data/spawn-and-move/Scarb.toml");

        let config = compiler::copy_tmp_config(
            &Utf8PathBuf::from("../../examples/spawn-and-move"),
            &Utf8PathBuf::from("../dojo-core"),
        );

        let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
        let dojo_metadata = dojo_metadata_from_workspace(&ws).expect(
            "No current package with dojo metadata found, bindgen is not yet supported for \
             workspaces.",
        );

        let data =
            gather_dojo_data(&manifest_path, "dojo_example", "dev", dojo_metadata.skip_migration)
                .unwrap();

        assert_eq!(data.models.len(), 8);

        assert_eq!(data.world.name, "dojo_example");

        let pos = data.models.get("Position").unwrap();
        assert_eq!(pos.name, "Position");
        assert_eq!(pos.qualified_path, "dojo_examples::models::Position");

        let moves = data.models.get("Moves").unwrap();
        assert_eq!(moves.name, "Moves");
        assert_eq!(moves.qualified_path, "dojo_examples::models::Moves");

        let moved = data.models.get("Message").unwrap();
        assert_eq!(moved.name, "Message");
        assert_eq!(moved.qualified_path, "dojo_examples::models::Message");

        let player_config = data.models.get("PlayerConfig").unwrap();
        assert_eq!(player_config.name, "PlayerConfig");
        assert_eq!(player_config.qualified_path, "dojo_examples::models::PlayerConfig");
    }
}
