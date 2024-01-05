use std::collections::HashMap;

use async_trait::async_trait;
use cainome::parser::tokens::Token;

use crate::error::BindgenResult;

pub mod typescript;
pub mod unity;

#[derive(Debug)]
pub enum Backend {
    Typescript,
    Unity,
}

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
