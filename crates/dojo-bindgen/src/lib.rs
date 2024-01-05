use std::collections::HashMap;
use std::fs;

use async_trait::async_trait;
use cainome::parser::tokens::Token;
use cainome::parser::AbiParser;
use camino::Utf8PathBuf;
use starknet::core::types::contract::{AbiEntry, SierraClass};

pub mod error;
use error::{BindgenResult, Error};

mod backends;
use backends::typescript::TypescriptBuilder;
pub use backends::Backend;

#[async_trait]
pub trait BackendBuilder {
    /// Generates the bindings for all the systems found in the given contract.
    ///
    /// # Arguments
    ///
    /// * `contract_name` - Fully qualified name (with modules) of the contract.
    /// * `tokens` - Tokens extracted from the ABI of the contract.
    async fn generate_systems_bindings(
        &self,
        contract_name: &str,
        tokens: HashMap<String, Vec<Token>>,
    ) -> BindgenResult<()>;
}

// TODO: include the manifest to have more metadata?
#[derive(Debug)]
pub struct BindingManager {
    pub artifacts_path: Utf8PathBuf,
    pub backends: Vec<Backend>,
}

impl BindingManager {
    /// Generates the bindings for all the given [`Backend`].
    ///
    /// # Arguments
    ///
    /// * `artifacts_path` - Path of contracts artifacts.
    /// * `backends` - A list of backends for which bindings must be generated.
    pub async fn generate(&self) -> BindgenResult<()> {
        if self.backends.is_empty() {
            return Ok(());
        }

        println!("GENERATE {:?}", self);

        for backend in &self.backends {
            // Get the backend builder from the backend enum.
            let builder = match backend {
                Backend::Typescript => TypescriptBuilder::new(),
                Backend::Unity => todo!(),
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
/// * `abi` - A string representing the ABI
/// * `type_aliases` - Types to be renamed to avoid name clashing of generated types
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
/// * `abi` - A string representing the ABI
fn parse_abi_string(abi: &str) -> BindgenResult<Vec<AbiEntry>> {
    let entries = if let Ok(sierra) = serde_json::from_str::<SierraClass>(abi) {
        sierra.abi
    } else {
        serde_json::from_str::<Vec<AbiEntry>>(abi).map_err(Error::SerdeJson)?
    };

    Ok(entries)
}
