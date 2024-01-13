use std::collections::HashMap;

use async_trait::async_trait;
use cainome::parser::tokens::Token;

use crate::error::BindgenResult;
use crate::plugins::BuiltinPlugin;
use crate::{DojoMetadata, DojoModel};

pub struct TypescriptPlugin;

impl TypescriptPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BuiltinPlugin for TypescriptPlugin {
    async fn generate_models_bindings(
        &self,
        models: &HashMap<String, DojoModel>,
    ) -> BindgenResult<()> {
        println!("Typescript models bindings");
        for (name, model) in models {
            println!("## Model: {}\n", name);
            println!("{:?}\n", model);
        }

        Ok(())
    }

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
