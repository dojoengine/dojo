use anyhow::Result;
use clap::Args;
use dojo_utils::env::STARKNET_RPC_URL_ENV_VAR;
use dojo_world::config::Environment;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use tracing::trace;
use url::Url;

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Starknet options")]
pub struct StarknetOptions {
    #[arg(long, env = STARKNET_RPC_URL_ENV_VAR)]
    #[arg(value_name = "URL")]
    #[arg(help = "The Starknet RPC endpoint.")]
    #[arg(global = true)]
    pub rpc_url: Option<Url>,
}

impl StarknetOptions {
    pub fn provider(
        &self,
        env_metadata: Option<&Environment>,
    ) -> Result<JsonRpcClient<HttpTransport>> {
        let url = self.url(env_metadata)?;
        trace!(?url, "Creating JsonRpcClient with given RPC URL.");
        Ok(JsonRpcClient::new(HttpTransport::new(url)))
    }

    // We dont check the env var because that would be handled by `clap`.
    // This function is made public because [`JsonRpcClient`] does not expose
    // the raw rpc url.
    pub fn url(&self, env_metadata: Option<&Environment>) -> Result<Url> {
        trace!("Retrieving RPC URL for StarknetOptions.");
        if let Some(url) = self.rpc_url.as_ref() {
            trace!(?url, "Using RPC URL from command line.");
            Ok(url.clone())
        } else if let Some(url) = env_metadata.and_then(|env| env.rpc_url()) {
            trace!(url, "Using RPC URL from environment metadata.");
            Ok(Url::parse(url)?)
        } else {
            trace!("Using default RPC URL: http://localhost:5050.");
            Ok(Url::parse("http://localhost:5050").unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use dojo_utils::env::STARKNET_RPC_URL_ENV_VAR;

    use super::StarknetOptions;

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
        let env_metadata = dojo_world::config::Environment {
            rpc_url: Some(METADATA_RPC.into()),
            ..Default::default()
        };

        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), METADATA_RPC);
    }

    #[test]
    fn url_doesnt_exist_in_env_but_exist_in_args() {
        let env_metadata = dojo_world::config::Environment::default();
        let cmd = Command::parse_from(["sozo", "--rpc-url", ENV_RPC]);

        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), ENV_RPC);
    }

    #[test]
    fn url_exists_in_both() {
        let env_metadata = dojo_world::config::Environment {
            rpc_url: Some(METADATA_RPC.into()),
            ..Default::default()
        };

        let cmd = Command::parse_from(["sozo", "--rpc-url", ENV_RPC]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), ENV_RPC);
    }

    #[test]
    fn url_exists_in_neither() {
        let env_metadata = dojo_world::config::Environment::default();
        let cmd = Command::parse_from([""]);
        assert_eq!(cmd.options.url(Some(&env_metadata)).unwrap().as_str(), DEFAULT_RPC);
    }
}
