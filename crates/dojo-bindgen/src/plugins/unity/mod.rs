use std::collections::HashMap;

use async_trait::async_trait;
use cainome::parser::tokens::Token;

use crate::error::BindgenResult;
use crate::plugins::BuiltinPlugin;

pub struct UnityPlugin;

impl UnityPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BuiltinPlugin for UnityPlugin {
    async fn generate_systems_bindings(
        &self,
        contract_name: &str,
        tokens: HashMap<String, Vec<Token>>,
    ) -> BindgenResult<()> {
        println!("Unity bindings for {}\n{:?}", contract_name, tokens);

        Ok(())
    }
}
