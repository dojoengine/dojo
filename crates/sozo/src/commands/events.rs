use crate::ops::events;

use super::options::{dojo_metadata_from_workspace, starknet::StarknetOptions};
use anyhow::Result;
use clap::Args;
use scarb::core::Config;

#[derive(Args, Debug)]
pub struct EventsArgs {
    #[clap(short, long)]
    #[clap(help = "idk yet")]
    pub chunk_size: u64,

    #[command(flatten)]
    pub starknet: StarknetOptions,
}

impl EventsArgs {
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
        config.tokio_handle().block_on(events::execute(self, env_metadata))
    }
}
