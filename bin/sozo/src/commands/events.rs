use anyhow::Result;
use clap::Parser;
use scarb::core::Config;
use sozo_ops::events;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Parser, Debug)]
pub struct EventsArgs {
    #[arg(help = "List of specific events to be filtered")]
    #[arg(value_delimiter = ',')]
    pub events: Option<Vec<String>>,

    #[arg(short, long)]
    #[arg(help = "Block number from where to look for events")]
    pub from_block: Option<u64>,

    #[arg(short, long)]
    #[arg(help = "Block number until where to look for events")]
    pub to_block: Option<u64>,

    #[arg(short, long)]
    #[arg(help = "Number of events to return per page")]
    pub chunk_size: u64,

    #[arg(long)]
    #[arg(help = "Continuation string to be passed for rpc request")]
    pub continuation_token: Option<String>,

    #[arg(long)]
    #[arg(help = "Print values as raw json")]
    pub json: bool,

    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,
}

impl EventsArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let env_metadata = utils::load_metadata_from_config(config)?;
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
        let provider = self.starknet.provider(env_metadata.as_ref())?;

        let event_filter = events::get_event_filter(
            self.from_block,
            self.to_block,
            self.events,
            self.world.world_address,
        );

        config.tokio_handle().block_on(async {
            events::parse(
                self.chunk_size,
                provider,
                self.continuation_token,
                event_filter,
                self.json,
                &manifest_dir,
            )
            .await
        })
    }
}
