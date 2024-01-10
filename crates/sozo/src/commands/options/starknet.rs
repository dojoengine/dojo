use anyhow::Result;
use clap::Args;
use dojo_world::metadata::Environment;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use url::Url;

use super::STARKNET_RPC_URL_ENV_VAR;

#[derive(Debug, Args)]
#[command(next_help_heading = "Starknet options")]
pub struct StarknetOptions {
    #[arg(long, env = STARKNET_RPC_URL_ENV_VAR)]
    #[arg(value_name = "URL")]
    #[arg(help = "The Starknet RPC endpoint.")]
    pub rpc_url: Option<Url>,
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
        if let Some(url) = self.rpc_url.as_ref() {
            Ok(url.clone())
        } else if let Some(url) = env_metadata.and_then(|env| env.rpc_url()) {
            Ok(Url::parse(url)?)
        } else {
            Ok(Url::parse("http://localhost:5050").unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::StarknetOptions;
    use crate::commands::options::STARKNET_RPC_URL_ENV_VAR;

    const ENV_RPC: &str = "http://localhost:7474/";
    const METADATA_RPC: &str = "http://localhost:6060/";
    const DEFAULT_RPC: &str = "http://localhost:5050/";

    #[derive(clap::Parser)]
    struct Command {
        #[clap(flatten)]
        options: StarknetOptions,
    }

    #[test]
    fn url_read_from_env_variable() {
        std::env::set_var(STARKNET_RPC_URL_ENV_VAR, ENV_RPC);

        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(None).unwrap().as_str(), ENV_RPC);
    }

    #[test]
    fn url_exist_in_env_but_not_in_args() {
        let env_metadata = dojo_world::metadata::Environment {
            rpc_url: Some(METADATA_RPC.into()),
            ..Default::default()
        };

        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), METADATA_RPC);
    }

    #[test]
    fn url_doesnt_exist_in_env_but_exist_in_args() {
        let env_metadata = dojo_world::metadata::Environment::default();
        let cmd = Command::parse_from(["sozo", "--rpc-url", ENV_RPC]);

        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), ENV_RPC);
    }

    #[test]
    fn url_exists_in_both() {
        let env_metadata = dojo_world::metadata::Environment {
            rpc_url: Some(METADATA_RPC.into()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--rpc-url", ENV_RPC]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), ENV_RPC);
    }

    #[test]
    fn url_exists_in_neither() {
        let env_metadata = dojo_world::metadata::Environment::default();
        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), DEFAULT_RPC);
    }
}
