use std::collections::HashMap;

use async_trait::async_trait;
use cainome::parser::tokens::Token;

use crate::error::BindgenResult;
use crate::BackendBuilder;

pub struct TypescriptBuilder;

impl TypescriptBuilder {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BackendBuilder for TypescriptBuilder {
    async fn generate_systems_bindings(
        &self,
        contract_name: &str,
        tokens: HashMap<String, Vec<Token>>,
    ) -> BindgenResult<()> {
        println!("Typescript systems bindings for {}", contract_name);
        println!("{:?}", tokens);
        Ok(())
    }
}
