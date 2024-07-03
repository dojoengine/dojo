use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use cainome::parser::tokens::Token;
use cainome::parser::{AbiParser, TokenizedAbi};
use camino::Utf8PathBuf;
use dojo_world::manifest::BaseManifest;
pub mod error;
use error::BindgenResult;

mod plugins;
use plugins::typescript::TypescriptPlugin;
use plugins::typescript_v2::TypeScriptV2Plugin;
use plugins::unity::UnityPlugin;
use plugins::BuiltinPlugin;
pub use plugins::BuiltinPlugins;

#[derive(Debug, PartialEq)]
pub struct DojoModel {
    /// model tag.
    pub tag: String,
    /// List of tokens found in the model contract ABI.
    /// Only structs and enums are currently used.
    pub tokens: TokenizedAbi,
}

#[derive(Debug, PartialEq)]
pub struct DojoContract {
    /// Contract tag.
    pub tag: String,
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
        base_manifest.remove_tags(skip_manifests);
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
        let tag = contract_manifest.inner.tag.clone();

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

        contracts.insert(tag.clone(), DojoContract { tag, tokens, systems });
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
        let tag = model_manifest.inner.tag.clone();

        let model = DojoModel { tag: tag.clone(), tokens: filter_model_tokens(&tokens) };

        models.insert(tag.clone(), model);
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

#[cfg(test)]
mod tests {
    use dojo_test_utils::compiler;
    use dojo_world::metadata::dojo_metadata_from_workspace;

    use super::*;

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

        assert_eq!(data.models.len(), 7);

        assert_eq!(data.world.name, "dojo_example");

        let pos = data.models.get("Position").unwrap();
        assert_eq!(pos.tag, "Position");

        let moves = data.models.get("Moves").unwrap();
        assert_eq!(moves.tag, "Moves");

        let moved = data.models.get("Message").unwrap();
        assert_eq!(moved.tag, "Message");

        let player_config = data.models.get("PlayerConfig").unwrap();
        assert_eq!(player_config.tag, "PlayerConfig");
    }
}
