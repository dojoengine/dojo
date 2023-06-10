use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use starknet::core::types::FieldElement;
use toml::Value;

#[derive(Debug, Args)]
#[command(next_help_heading = "World options")]
pub struct WorldOptions {
    #[arg(long = "world")]
    #[arg(help = "The address of the World contract.")]
    pub world_address: Option<FieldElement>,
}

impl WorldOptions {
    pub fn address(&self, env_metadata: Option<&Value>) -> Result<FieldElement> {
        if let Some(world_address) = self.world_address {
            return Ok(world_address);
        } else if let Some(dojo_metadata) = env_metadata {
            if let Some(world_address) = dojo_metadata.get("world_address") {
                if let Some(world_address) = world_address.as_str() {
                    let world_address = FieldElement::from_str(world_address)?;
                    return Ok(world_address);
                }
            }
        }

        Err(anyhow!(
            "Could not find World address. Please specify it with --world or in the world config."
        ))
    }
}
