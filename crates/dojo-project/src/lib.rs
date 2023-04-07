pub mod migration;

use scarb::core::Workspace;
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use toml::Value;

#[allow(clippy::enum_variant_names)]
#[derive(thiserror::Error, Debug)]
pub enum DeserializationError {
    #[error("parsing field element")]
    ParsingFieldElement,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldConfig {
    pub address: Option<FieldElement>,
}

pub struct DeploymentConfig {
    pub rpc: Option<String>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployments {
    pub testnet: Option<Deployment>,
    pub mainnet: Option<Deployment>,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    pub rpc: Option<String>,
}

fn dojo_metadata_from_workspace(ws: &Workspace<'_>) -> Option<Value> {
    ws.current_package().ok()?.manifest.metadata.tool_metadata.as_ref()?.get("dojo").cloned()
}

impl WorldConfig {
    pub fn from_workspace(ws: &Workspace<'_>) -> Result<Self, DeserializationError> {
        let mut world_config = WorldConfig::default();

        if let Some(dojo_metadata) = dojo_metadata_from_workspace(ws) {
            if let Some(world_address) = dojo_metadata.get("world_address") {
                if let Some(world_address) = world_address.as_str() {
                    let world_address = FieldElement::from_hex_be(world_address)
                        .map_err(|_| DeserializationError::ParsingFieldElement)?;
                    world_config.address = Some(world_address);
                }
            }
        }

        Ok(world_config)
    }
}
