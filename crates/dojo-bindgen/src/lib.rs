use std::collections::HashMap;
use std::fs;

use cainome::parser::tokens::Token;
use cainome::parser::AbiParser;
use camino::Utf8PathBuf;
use starknet::core::types::contract::{AbiEntry, SierraClass};

pub mod error;
use error::{BindgenResult, Error};

mod plugins;
use plugins::typescript::TypescriptPlugin;
use plugins::unity::UnityPlugin;
use plugins::BuiltinPlugin;
pub use plugins::BuiltinPlugins;

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
    /// Generates the bindings for all the given [`Plugin`].
    pub async fn generate(&self) -> BindgenResult<()> {
        if self.builtin_plugins.is_empty() && self.plugins.is_empty() {
            return Ok(());
        }

        println!("Generating bindings {:?}", self);

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
                            let tokens = tokens_from_abi_string(&file_content, &types_aliases)?;
                            builder.generate_systems_bindings(file_name, tokens).await?;
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

/// Generates the [`Token`]s from the given ABI string.
///
/// The `abi` can have two formats:
/// 1. Entire [`SierraClass`] json representation.
/// 2. The `abi` key from the [`SierraClass`], which is an array of [`AbiEntry`].
///
/// TODO: Move to cainome implementation when available.
///
/// # Arguments
///
/// * `abi` - A string representing the ABI.
/// * `type_aliases` - Types to be renamed to avoid name clashing of generated types.
fn tokens_from_abi_string(
    abi: &str,
    type_aliases: &HashMap<String, String>,
) -> BindgenResult<HashMap<String, Vec<Token>>> {
    let abi_entries = parse_abi_string(abi)?;
    let abi_tokens = AbiParser::collect_tokens(&abi_entries).expect("failed tokens parsing");
    let abi_tokens = AbiParser::organize_tokens(abi_tokens, type_aliases);

    Ok(abi_tokens)
}

/// Parses an ABI string to output a [`Vec<AbiEntry`].
///
/// The `abi` can have two formats:
/// 1. Entire [`SierraClass`] json representation.
/// 2. The `abi` key from the [`SierraClass`], which is an array of [`AbiEntry`].
///
/// TODO: Move to cainome implementation when available.
///
/// # Arguments
///
/// * `abi` - A string representing the ABI.
fn parse_abi_string(abi: &str) -> BindgenResult<Vec<AbiEntry>> {
    let entries = if let Ok(sierra) = serde_json::from_str::<SierraClass>(abi) {
        sierra.abi
    } else {
        serde_json::from_str::<Vec<AbiEntry>>(abi).map_err(Error::SerdeJson)?
    };

    Ok(entries)
}
