use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use cainome::parser::tokens::Token;
use cainome::parser::{AbiParser, TokenizedAbi};
use camino::Utf8PathBuf;
use dojo_world::config::ProfileConfig;
pub mod error;
use dojo_world::local::{ResourceLocal, WorldLocal};
use error::BindgenResult;

mod plugins;
use plugins::recs::TypescriptRecsPlugin;
use plugins::typescript::TypescriptPlugin;
use plugins::unity::UnityPlugin;
use plugins::unrealengine::UnrealEnginePlugin;
use plugins::BuiltinPlugin;
pub use plugins::BuiltinPlugins;

use crate::error::Error;

#[derive(Debug, PartialEq)]
pub struct DojoModel {
    /// model tag.
    pub tag: String,
    /// List of tokens found in the model contract ABI.
    /// Only structs and enums are currently used.
    pub tokens: TokenizedAbi,
}

#[derive(Debug, PartialEq)]
pub struct DojoEvent {
    /// event tag.
    pub tag: String,
    /// List of tokens found in the event contract ABI.
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
    /// All the events contracts found in the project.
    pub events: HashMap<String, DojoEvent>,
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
                BuiltinPlugins::UnrealEngine => Box::new(UnrealEnginePlugin::new()),
                BuiltinPlugins::Recs => Box::new(TypescriptRecsPlugin::new()),
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

    let profile_config =
        ProfileConfig::from_toml(root_dir.join(format!("dojo_{}.toml", profile_name)))?;
    let target_dir = root_dir.join("target").join(profile_name);

    if !target_dir.exists() {
        return Err(Error::GatherDojoData(format!(
            "Target directory does not exist. Ensure you've built the project before generating \
             bindings. Target directory: {target_dir}"
        )));
    }

    let world_local = WorldLocal::from_directory(&target_dir, profile_name, profile_config)?;

    let mut models = HashMap::new();
    let mut contracts = HashMap::new();
    let mut events = HashMap::new();

    for r in world_local.resources.values() {
        if let Some(skip_migrations) = &skip_migration {
            if skip_migrations.contains(&r.tag()) {
                continue;
            }
        }

        match r {
            ResourceLocal::Contract(c) => {
                let tokens = AbiParser::collect_tokens(&c.common.class.abi, &HashMap::new())?;

                // Identify the systems -> for now only take the functions from the
                // interfaces.
                let mut systems = vec![];
                let interface_blacklist =
                    ["dojo::world::IWorldProvider", "dojo::contract::upgradeable::IUpgradeable"];

                // Blacklist all the functions that are added by Dojo macros.
                let function_blacklist = ["dojo_init", "upgrade", "world_dispatcher", "dojo_name"];

                for (interface, funcs) in &tokens.interfaces {
                    if !interface_blacklist.contains(&interface.as_str()) {
                        for func in funcs {
                            if !function_blacklist
                                .contains(&func.to_function().unwrap().name.as_str())
                            {
                                systems.push(func.clone());
                            }
                        }
                    }
                }

                for func in &tokens.functions {
                    if !function_blacklist.contains(&func.to_function().unwrap().name.as_str()) {
                        systems.push(func.clone());
                    }
                }

                let tag = r.tag();

                contracts.insert(tag.clone(), DojoContract { tag, tokens, systems });
            }
            ResourceLocal::Model(m) => {
                let tokens = AbiParser::collect_tokens(&m.common.class.abi, &HashMap::new())?;
                let tag = r.tag();
                models.insert(tag.clone(), DojoModel { tag, tokens: filter_model_tokens(&tokens) });
            }
            ResourceLocal::Event(m) => {
                let tokens = AbiParser::collect_tokens(&m.common.class.abi, &HashMap::new())?;
                let tag = r.tag();
                events.insert(tag.clone(), DojoEvent { tag, tokens: filter_model_tokens(&tokens) });
            }
            _ => {}
        }
    }

    let world = DojoWorld { name: root_package_name.to_string() };

    Ok(DojoData { world, models, contracts, events })
}

/// Filters the model ABI to keep relevant types
/// to be generated for bindings.
fn filter_model_tokens(tokens: &TokenizedAbi) -> TokenizedAbi {
    let mut structs = vec![];
    let mut enums = vec![];

    // All types from introspect module can also be removed as the clients does not rely on them.
    // Events are also always empty at model contract level.
    fn skip_token(token: &Token) -> bool {
        if token.type_path().starts_with("dojo::model::introspect") {
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

/// Compares two tokens by their type name.
pub fn compare_tokens_by_type_name(a: &Token, b: &Token) -> Ordering {
    let a_name = a.to_composite().expect("composite expected").type_name_or_alias();
    let b_name = b.to_composite().expect("composite expected").type_name_or_alias();
    a_name.cmp(&b_name)
}

#[cfg(test)]
mod tests {
    use dojo_test_utils::setup::TestSetup;
    use scarb_interop::Profile;
    use scarb_metadata_ext::MetadataDojoExt;

    use super::*;

    #[test]
    fn gather_data_ok() {
        let setup = TestSetup::from_examples("../core", "../../../examples/");
        let metadata = setup.load_metadata("spawn-and-move", Profile::DEV);

        let profile_config = metadata.load_dojo_profile_config().unwrap();

        let skip_migrations = if let Some(migration) = profile_config.migration {
            let mut skip_migration = vec![];
            if let Some(skip_contracts) = migration.skip_contracts {
                skip_migration.extend(skip_contracts);
            }
            Some(skip_migration)
        } else {
            None
        };

        dbg!(&setup.manifest_path("spawn-and-move"));
        let data =
            gather_dojo_data(setup.manifest_path("spawn-and-move"), "ns", "dev", skip_migrations)
                .expect("Failed to gather dojo data");

        assert_eq!(data.models.len(), 10);

        assert_eq!(data.world.name, "ns");

        let pos = data.models.get("ns-Position").unwrap();
        assert_eq!(pos.tag, "ns-Position");

        let moves = data.models.get("ns-Moves").unwrap();
        assert_eq!(moves.tag, "ns-Moves");

        let moved = data.models.get("ns-Message").unwrap();
        assert_eq!(moved.tag, "ns-Message");

        let player_config = data.models.get("ns-PlayerConfig").unwrap();
        assert_eq!(player_config.tag, "ns-PlayerConfig");
    }
}
