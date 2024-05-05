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
        trace!(?env_metadata, "Metadata loaded from config.");

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        trace!(ws_members_count = ws.members().count(), "Read workspace.");

        let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
        trace!(?manifest_dir, "Manifest directory defined from workspace.");

        let provider = self.starknet.provider(env_metadata.as_ref())?;
        trace!(?provider, "Starknet RPC client provider.");

        let event_filter = events::get_event_filter(
            self.from_block,
            self.to_block,
            self.events,
            self.world.world_address,
        );
        trace!(
            from_block = self.from_block,
            to_block = self.to_block,
            chunk_size = self.chunk_size,
            "Created event filter."
        );
        let profile_name =
            ws.current_profile().expect("Scarb profile expected at this point.").to_string();
        trace!(profile_name, "Current profile.");

        config.tokio_handle().block_on(async {
            trace!("Starting async event parsing.");
            events::parse(
                self.chunk_size,
                provider,
                self.continuation_token,
                event_filter,
                self.json,
                &manifest_dir,
                &profile_name,
            )
            .await
        })
    }
}
