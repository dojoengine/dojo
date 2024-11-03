use anyhow::Result;
use clap::Args;
use scarb::core::Config;
use sozo_ops::events;
use tracing::trace;

use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
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
    #[arg(default_value_t = 100)]
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
        config.tokio_handle().block_on(async {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

            let (world_diff, provider, _) = utils::get_world_diff_and_provider(
                self.starknet,
                self.world,
                &ws,
            )
            .await?;

            let event_filter = events::get_event_filter(
                self.from_block,
                self.to_block,
                self.events,
                Some(world_diff.world_info.address),
            );

            trace!("Starting async event parsing.");
            events::parse(
                &world_diff,
                &provider,
                self.chunk_size,
                self.continuation_token,
                event_filter,
            )
            .await
        })
    }
}
