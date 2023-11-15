use anyhow::{anyhow, Result};
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use url::Url;

#[derive(Debug, Args)]
#[command(next_help_heading = "Starknet options")]
pub struct StarknetOptions {
    #[arg(long, env = "STARKNET_RPC_URL", default_value = "http://localhost:5050")]
    #[arg(value_name = "URL")]
    #[arg(help = "The Starknet RPC endpoint.")]
    pub rpc_url: Url,
}

impl StarknetOptions {
    pub fn provider(
        &self,
        env_metadata: Option<&Environment>,
    ) -> Result<JsonRpcClient<HttpTransport>> {
        let url = if let Some(url) = env_metadata.and_then(|env| env.rpc_url()) {
            Some(Url::parse(url)?)
        } else if let Some(url) = std::env::var("STARKNET_RPC_URL").ok().as_deref() {
            Some(Url::parse(url)?)
        } else {
            Some(self.rpc_url.clone())
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
