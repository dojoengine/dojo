use anyhow::{anyhow, Result};
use clap::Args;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use toml::Value;
use url::Url;

#[derive(Debug, Args)]
#[command(next_help_heading = "Starknet options")]
pub struct StarknetOptions {
    #[arg(long)]
    #[arg(value_name = "URL")]
    #[arg(help = "The Starknet RPC endpoint.")]
    pub rpc_url: Option<Url>,
}

impl StarknetOptions {
    pub fn provider(&self, env_metadata: Option<&Value>) -> Result<JsonRpcClient<HttpTransport>> {
        let url = if let Some(url) = self.rpc_url.clone() {
            Some(url)
        } else if let Some(url) = env_metadata
            .and_then(|env| env.get("rpc_url").and_then(|v| v.as_str().map(|s| s.to_string())))
            .or(std::env::var("STARKNET_RPC_URL").ok())
        {
            Some(Url::parse(&url)?)
        } else {
            None
        };

        if let Some(url) = url {
            Ok(JsonRpcClient::new(HttpTransport::new(url)))
        } else {
            Err(anyhow!(
                "Could not find Starknet RPC endpoint. Please specify it with --rpc-url or in the \
                 environment config."
            ))
        }
    }
}
