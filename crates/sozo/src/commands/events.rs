use anyhow::Result;
use clap::Parser;
use scarb::core::Config;

use super::options::dojo_metadata_from_workspace;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use crate::ops::events;

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

    #[command(flatten)]
    pub world: WorldOptions,

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

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn events_are_parsed_correctly() {
        let arg = EventsArgs::parse_from(["event", "Event1,Event2", "--chunk-size", "1"]);
        assert!(arg.events.unwrap().len() == 2);
        assert!(arg.from_block.is_none());
        assert!(arg.to_block.is_none());
        assert!(arg.chunk_size == 1);
    }
}
