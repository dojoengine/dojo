use anyhow::anyhow;
use scarb::core::Workspace;
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use toml::Value;
use tracing::warn;
use url::Url;

pub mod manifest;
pub mod migration;

#[cfg(test)]
mod test_utils;

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
    pub private_key: Option<FieldElement>,
    pub account_address: Option<FieldElement>,
}

impl EnvironmentConfig {
    pub fn from_workspace<T: AsRef<str>>(profile: T, ws: &Workspace<'_>) -> anyhow::Result<Self> {
        let mut config = EnvironmentConfig::default();

        let mut env_metadata = dojo_metadata_from_workspace(ws)
            .and_then(|dojo_metadata| dojo_metadata.get("env").cloned());

        // If there is an environment-specific metadata, use that, otherwise use the
        // workspace's default environment metadata.
        env_metadata = env_metadata
            .as_ref()
            .and_then(|env_metadata| env_metadata.get(profile.as_ref()).cloned())
            .or(env_metadata);

        if let Some(env) = env_metadata {
            if let Some(rpc) = env.get("rpc_url").and_then(|v| v.as_str()) {
                let url = Url::parse(rpc).map_err(|_| DeserializationError::ParsingUrl)?;
                config.rpc = Some(url);
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
        }

        Ok(config)
    }

    pub fn provider(&self) -> anyhow::Result<JsonRpcClient<HttpTransport>> {
        if self.rpc.is_none() && self.network.is_none() {
            return Err(anyhow!("Missing `rpc_url` or `network` in the environment config"));
        }

        if self.rpc.is_some() && self.network.is_some() {
            warn!("Both `rpc_url` and `network` are set but `rpc_url` will be used instead")
        }

        let provider = if let Some(url) = &self.rpc {
            JsonRpcClient::new(HttpTransport::new(url.clone()))
        } else {
            match self.network.as_ref().unwrap().as_str() {
                "mainnet" => JsonRpcClient::new(HttpTransport::new(
                    Url::parse(
                        "https://starknet-goerli.g.alchemy.com/v2/KE9ZWlO2zAaXFvpjbyb63gZIX1SozzON",
                    )
                    .unwrap(),
                )),
                "goerli" => JsonRpcClient::new(HttpTransport::new(
                    Url::parse(
                        "https://starknet-mainnet.g.alchemy.com/v2/qnYLy7taPFweUC6wad3qF7-bCb4YnQN4",
                    )
                    .unwrap(),
                )),
                n => return Err(anyhow!("Unsupported network: {n}")),
            }
        };

        Ok(provider)
    }
}
