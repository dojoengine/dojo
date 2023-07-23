use anyhow::Result;
use clap::Args;
use scarb::core::Config;
use starknet::core::types::FieldElement;

use super::options::account::AccountOptions;
use super::options::dojo_metadata_from_workspace;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::ops::execute;

#[derive(Debug, Args)]
#[command(about = "Execute a system with the given calldata.")]
pub struct ExecuteArgs {
    #[arg(help = "The name of the system to be executed.")]
    pub system: String,

    #[arg(short, long)]
    #[arg(value_delimiter = ',')]
    #[arg(help = "The calldata to be passed to the system. Comma seperated values e.g., \
                  0x12345,0x69420.")]
    pub calldata: Vec<FieldElement>,

    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,
}

impl ExecuteArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
            let env_metadata = dojo_metadata_from_workspace(&ws)
                .and_then(|dojo_metadata| dojo_metadata.get("env").cloned());

            env_metadata
                .as_ref()
                .and_then(|env_metadata| env_metadata.get(ws.config().profile().as_str()).cloned())
                .or(env_metadata)
        } else {
            None
        };

        config.tokio_handle().block_on(execute::execute(self, env_metadata))
    }
}
