use std::collections::HashMap;
use std::fs;

use cainome::parser::tokens::Token;
use cainome::parser::AbiParser;
use camino::Utf8PathBuf;
use starknet::core::types::contract::{AbiEntry, SierraClass};

pub mod error;
use error::{BindgenResult, Error};

mod backends;
use backends::typescript::TypescriptBuilder;
use backends::unity::UnityBuilder;
pub use backends::Backend;
use backends::BackendBuilder;

// TODO: include the manifest to have more metadata?
#[derive(Debug)]
pub struct BindingManager {
    /// Path of contracts artifacts.
    pub artifacts_path: Utf8PathBuf,
    /// A list of backends for which bindings must be generated.
    pub backends: Vec<Backend>,
}

impl BindingManager {
    /// Generates the bindings for all the given [`Backend`].
    pub async fn generate(&self) -> BindgenResult<()> {
        if self.backends.is_empty() {
            return Ok(());
        }

        println!("Generating bindings with {:?}", self);

        for backend in &self.backends {
            // Get the backend builder from the backend enum.
            let builder: Box<dyn BackendBuilder> = match backend {
                Backend::Typescript => Box::new(TypescriptBuilder::new()),
                Backend::Unity => Box::new(UnityBuilder::new()),
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
    if file_name.starts_with("dojo") || file_name == "manifest.json" {
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
