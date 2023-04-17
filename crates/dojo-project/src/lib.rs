pub mod migration;

use std::str::FromStr;

use anyhow::anyhow;
use scarb::core::Workspace;
use serde::{Deserialize, Serialize};
use starknet::core::chain_id;
use starknet::core::types::FieldElement;
use starknet::providers::SequencerGatewayProvider;
use toml::Value;
use url::Url;

#[allow(clippy::enum_variant_names)]
#[derive(thiserror::Error, Debug)]
pub enum DeserializationError {
    #[error("parsing field element")]
    ParsingFieldElement,
    #[error("parsing url")]
    ParsingUrl,
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

#[derive(Debug, Deserialize, Clone, Default)]
pub struct EnvironmentConfig {
    pub rpc: Option<Url>,
    pub network: Option<String>,
    pub chain_id: Option<FieldElement>,
    pub private_key: Option<FieldElement>,
    pub account_address: Option<FieldElement>,
}

impl EnvironmentConfig {
    pub fn from_workspace<T: AsRef<str>>(
        env: T,
        ws: &Workspace<'_>,
    ) -> Result<Self, DeserializationError> {
        let mut config = EnvironmentConfig::default();

        if let Some(env) = dojo_metadata_from_workspace(ws)
            .and_then(|dojo_metadata| dojo_metadata.get("env").cloned())
            .and_then(|env_metadata| env_metadata.get(env.as_ref()).cloned())
        {
            if let Some(rpc) = env.get("rpc_url").and_then(|v| v.as_str()) {
                let url = Url::parse(rpc).map_err(|_| DeserializationError::ParsingUrl)?;
                config.rpc = Some(url);
            }

            if let Some(chain_id) = env.get("chain_id").and_then(|v| v.as_str()) {
                let chain_id = FieldElement::from_str(chain_id)
                    .map_err(|_| DeserializationError::ParsingFieldElement)?;
                config.chain_id = Some(chain_id);
            }

            if let Some(private_key) = env
                .get("private_key")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or(std::env::var("DOJO_PRIVATE_KEY").ok())
            {
                let pk = FieldElement::from_hex_be(&private_key)
                    .map_err(|_| DeserializationError::ParsingFieldElement)?;
                config.private_key = Some(pk);
            }

            if let Some(account_address) = env
                .get("account_address")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or(std::env::var("DOJO_ACCOUNT_ADDRESS").ok())
            {
                let address = FieldElement::from_hex_be(&account_address)
                    .map_err(|_| DeserializationError::ParsingFieldElement)?;
                config.account_address = Some(address);
            }

            if let Some(network) = env.get("network").and_then(|v| v.as_str()) {
                config.network = Some(network.into());
                if config.chain_id.is_none() {
                    config.chain_id = get_chain_id_from_network(network);
                }
            }
        }

        Ok(config)
    }

    pub fn get_provider(&self) -> anyhow::Result<SequencerGatewayProvider> {
        if self.rpc.is_none() && self.network.is_none() {
            return Err(anyhow!("Missing `rpc_url` or `network` in the environment config"));
        }

        let provider = if let Some(url) = &self.rpc {
            let mut base = url.clone();
            base.path_segments_mut().unwrap().pop_if_empty();

            let mut gateway = base.clone();
            gateway.path_segments_mut().unwrap().push("gateway");
            let mut feeder_gateway = base.clone();
            feeder_gateway.path_segments_mut().unwrap().push("feeder_gateway");

            SequencerGatewayProvider::new(gateway, feeder_gateway)
        } else {
            match self.network.as_ref().unwrap().as_str() {
                "mainnet" => SequencerGatewayProvider::starknet_alpha_mainnet(),
                "goerli" => SequencerGatewayProvider::starknet_alpha_goerli(),
                "goerli2" => SequencerGatewayProvider::starknet_alpha_goerli_2(),
                n => return Err(anyhow!("Unsupported network: {n}")),
            }
        };

        Ok(provider)
    }
}

fn get_chain_id_from_network(network: &str) -> Option<FieldElement> {
    match network {
        "mainnet" => Some(chain_id::MAINNET),
        "goerli" => Some(chain_id::TESTNET),
        "goerli2" => Some(chain_id::TESTNET2),
        _ => None,
    }
}
