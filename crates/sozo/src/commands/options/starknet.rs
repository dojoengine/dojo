use anyhow::Result;
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use url::Url;

const STARKNET_RPC_URL_ENV_VAR: &str = "STARKNET_RPC_URL";

#[derive(Debug, Args)]
#[command(next_help_heading = "Starknet options")]
pub struct StarknetOptions {
    #[arg(long, env = STARKNET_RPC_URL_ENV_VAR, default_value = "http://localhost:5050")]
    #[arg(value_name = "URL")]
    #[arg(help = "The Starknet RPC endpoint.")]
    pub rpc_url: Url,
}

impl StarknetOptions {
    pub fn provider(
        &self,
        env_metadata: Option<&Environment>,
    ) -> Result<JsonRpcClient<HttpTransport>> {
        Ok(JsonRpcClient::new(HttpTransport::new(self.url(env_metadata)?)))
    }

    // we dont check the env var because that would be handled by `clap`
    fn url(&self, env_metadata: Option<&Environment>) -> Result<Url> {
        Ok(if let Some(url) = env_metadata.and_then(|env| env.rpc_url()) {
            Url::parse(url)?
        } else {
            self.rpc_url.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::StarknetOptions;
    use crate::commands::options::starknet::STARKNET_RPC_URL_ENV_VAR;

    const ENV_RPC: &str = "http://localhost:7474/";
    const METADATA_RPC: &str = "http://localhost:6060/";

    #[derive(clap::Parser)]
    struct Command {
        #[clap(flatten)]
        options: StarknetOptions,
    }

    #[test]
    fn url_exist_in_env_metadata_but_env_doesnt() {
        let env_metadata = dojo_world::metadata::Environment {
            rpc_url: Some(METADATA_RPC.into()),
            ..Default::default()
        };

        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), METADATA_RPC);
    }

    #[test]
    fn url_doesnt_exist_in_env_metadata_but_env_does() {
        std::env::set_var(STARKNET_RPC_URL_ENV_VAR, ENV_RPC);
        let env_metadata = dojo_world::metadata::Environment::default();
        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), ENV_RPC);
    }

    #[test]
    fn exists_in_both() {
        std::env::set_var(STARKNET_RPC_URL_ENV_VAR, ENV_RPC);
        let env_metadata = dojo_world::metadata::Environment {
            rpc_url: Some(METADATA_RPC.into()),
            ..Default::default()
        };
        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), METADATA_RPC);
    }
}
