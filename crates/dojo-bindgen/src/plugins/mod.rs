use std::collections::HashMap;

use async_trait::async_trait;
use cainome::parser::tokens::Token;

use crate::error::BindgenResult;
use crate::{DojoMetadata, DojoModel};

pub mod typescript;
pub mod unity;

#[derive(Debug)]
pub enum BuiltinPlugins {
    Typescript,
    Unity,
}

#[async_trait]
pub trait BuiltinPlugin {
    /// Generates the models bindings.
    /// Each [`DojoModel`] contains all the types that are required to
    /// generate a model bindings.
    ///
    /// Warning, some types may be repeated in different models, due to the fact
    /// that the Cairo ABI contains all types used in a contract.
    ///
    /// It's at the plugin discretion to separate models bindings in modules/namespace
    /// to avoid collision, or ensuring unicity of a type among all the models.
    ///
    /// # Arguments
    ///
    /// * `models` - All the models found in the project.
    async fn generate_models_bindings(
        &self,
        models: &HashMap<String, DojoModel>,
    ) -> BindgenResult<()>;

    /// Generates the bindings for all the systems found in the given contract.
    /// The `tokens` fields is the self contained ABI, which means all the tokens
    /// to call any function in the contract is present in `tokens` ABI.
    ///
    /// A plugin may use the `generate_models_bindings` to centralized the models,
    /// bindings and ignore models type generation in this call.
    ///
    /// # Arguments
    ///
    /// * `contract_name` - Fully qualified name (with modules) of the contract.
    /// * `tokens` - Tokens extracted from the ABI of the contract.
    async fn generate_systems_bindings(
        &self,
        contract_name: &str,
        tokens: HashMap<String, Vec<Token>>,
        metadata: &DojoMetadata,
    ) -> BindgenResult<()>;
}

// TODO: define the Plugin interface communicating data via stdin.
// Data must be easily serializable to be deserialized on the plugin side.
// We need to define one PluginInput struct and one PluginOutput struct.
