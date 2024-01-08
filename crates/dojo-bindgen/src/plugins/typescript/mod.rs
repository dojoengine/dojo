use std::collections::HashMap;

use async_trait::async_trait;
use cainome::parser::tokens::Token;

use crate::error::BindgenResult;
use crate::plugins::BuiltinPlugin;
use crate::DojoMetadata;

pub struct TypescriptPlugin;

impl TypescriptPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BuiltinPlugin for TypescriptPlugin {
    async fn generate_systems_bindings(
        &self,
        contract_name: &str,
        tokens: HashMap<String, Vec<Token>>,
        metadata: &DojoMetadata,
    ) -> BindgenResult<()> {
        println!("Typescript bindings for {}\n{:?}\n{:?}", contract_name, metadata, tokens);

        Ok(())
    }
}
