use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::core::types::FieldElement;

#[derive(Debug, Args)]
#[command(next_help_heading = "World options")]
pub struct WorldOptions {
    #[arg(long = "world")]
    #[arg(help = "The address of the World contract.")]
    pub world_address: Option<FieldElement>,
}

impl WorldOptions {
    pub fn address(&self, env_metadata: Option<&Environment>) -> Result<FieldElement> {
        if let Some(world_address) = self.world_address {
            Ok(world_address)
        } else if let Some(world_address) = env_metadata
            .and_then(|env| env.world_address())
            .or(std::env::var("DOJO_WORLD_ADDRESS").ok().as_deref())
        {
            Ok(FieldElement::from_str(world_address)?)
        } else {
            Err(anyhow!(
                "Could not find World address. Please specify it with --world or in the world \
                 config."
            ))
        }
    }
}
