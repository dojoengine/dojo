use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;

use crate::error::BindgenResult;
use crate::plugins::BuiltinPlugin;
use crate::DojoData;

pub struct TypescriptPlugin;

impl TypescriptPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BuiltinPlugin for TypescriptPlugin {
    async fn generate_code(&self, data: &DojoData) -> BindgenResult<HashMap<PathBuf, Vec<u8>>> {
        println!("-> Typescript models bindings\n");

        for (name, model) in &data.models {
            println!("## Model: {}", name);
            println!("{:?}\n", model);
        }

        for (file_name, contract) in &data.contracts {
            println!("## Contract: {}", file_name);
            println!("{:?}\n", contract);
        }

        Ok(HashMap::new())
    }
}
