use async_trait::async_trait;

use crate::error::BindgenResult;
use crate::plugins::BuiltinPlugin;
use crate::DojoData;

pub struct UnityPlugin;

impl UnityPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl BuiltinPlugin for UnityPlugin {
    async fn generate_code(&self, data: &DojoData) -> BindgenResult<()> {
        println!("-> Unity models bindings\n");

        for (name, model) in &data.models {
            println!("## Model: {}", name);
            println!("{:?}\n", model);
        }

        for (file_name, contract) in &data.contracts {
            println!("## Contract: {}", file_name);
            println!("{:?}\n", contract);
        }

        Ok(())
    }
}
