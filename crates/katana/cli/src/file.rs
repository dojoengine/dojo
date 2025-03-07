use std::path::{Path, PathBuf};

use anyhow::Result;
use katana_messaging::MessagingConfig;
use serde::{Deserialize, Serialize};

use crate::options::*;
use crate::NodeArgs;

/// Node arguments configuration file.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NodeArgsConfig {
    pub no_mining: Option<bool>,
    pub block_time: Option<u64>,
    pub block_cairo_steps_limit: Option<u64>,
    pub db_dir: Option<PathBuf>,
    pub messaging: Option<MessagingConfig>,
    pub logging: Option<LoggingOptions>,
    pub starknet: Option<StarknetOptions>,
    pub gpo: Option<GasPriceOracleOptions>,
    pub forking: Option<ForkingOptions>,
    #[serde(rename = "dev")]
    pub development: Option<DevOptions>,
    #[cfg(feature = "server")]
    pub server: Option<ServerOptions>,
    #[cfg(feature = "server")]
    pub rpc: Option<RpcOptions>,
    #[cfg(feature = "server")]
    pub metrics: Option<MetricsOptions>,
}

impl NodeArgsConfig {
    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let file = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&file)?)
    }
}

impl TryFrom<NodeArgs> for NodeArgsConfig {
    type Error = anyhow::Error;

    fn try_from(args: NodeArgs) -> Result<Self> {
        // Ensure the config file is merged with the CLI arguments.
        let args = args.with_config_file()?;

        let mut node_config = NodeArgsConfig {
            no_mining: if args.no_mining { Some(true) } else { None },
            block_time: args.block_time,
            block_cairo_steps_limit: args.block_cairo_steps_limit,
            db_dir: args.db_dir,
            messaging: args.messaging,
            ..Default::default()
        };

        // Only include the following options if they are not the default.
        // This makes the config file more readable.
        node_config.logging =
            if args.logging == LoggingOptions::default() { None } else { Some(args.logging) };
        node_config.starknet =
            if args.starknet == StarknetOptions::default() { None } else { Some(args.starknet) };
        node_config.gpo =
            if args.gpo == GasPriceOracleOptions::default() { None } else { Some(args.gpo) };
        node_config.forking =
            if args.forking == ForkingOptions::default() { None } else { Some(args.forking) };
        node_config.development =
            if args.development == DevOptions::default() { None } else { Some(args.development) };

        #[cfg(feature = "server")]
        {
            node_config.server =
                if args.server == ServerOptions::default() { None } else { Some(args.server) };
            node_config.rpc = if args.rpc == RpcOptions::default() { None } else { Some(args.rpc) };
            node_config.metrics =
                if args.metrics == MetricsOptions::default() { None } else { Some(args.metrics) };
        }

        Ok(node_config)
    }
}
