use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use scarb::core::Workspace;
use starknet::core::types::FieldElement;

use super::dojo_metadata_from_workspace;

#[derive(Debug, Args)]
pub struct WorldOptions {
    #[arg(long = "world")]
    #[arg(help = "The address of the World contract")]
    #[arg(long_help = "")]
    pub world_address: Option<FieldElement>,
}

impl WorldOptions {
    pub fn address(&self, ws: &Workspace<'_>) -> Result<FieldElement> {
        if let Some(world_address) = self.world_address {
            return Ok(world_address);
        } else {
            if let Some(dojo_metadata) = dojo_metadata_from_workspace(ws) {
                if let Some(world_address) = dojo_metadata.get("world_address") {
                    if let Some(world_address) = world_address.as_str() {
                        let world_address = FieldElement::from_str(world_address)?;
                        return Ok(world_address);
                    }
                }
            }
        }

        Err(anyhow!(
            "Could not find World address. Please specify it with --world or in the world config."
        ))
    }
}
