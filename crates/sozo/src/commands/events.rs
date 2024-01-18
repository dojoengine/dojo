use std::collections::HashMap;

use anyhow::{anyhow, Result};
use cairo_lang_starknet::abi::{self, Event, Item};
use clap::Parser;
use dojo_world::manifest::Manifest;
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::Config;
use starknet::core::utils::starknet_keccak;

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
        let event_map = if !self.json {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
            let target_dir = ws.target_dir().path_existent()?;
            let manifest_path = target_dir.join(config.profile().as_str()).join("manifest.json");

            if !manifest_path.exists() {
                return Err(anyhow!("Run scarb migrate before running this command"));
            }

            Some(extract_events(&Manifest::load_from_path(manifest_path)?))
        } else {
            None
        };

        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

            // TODO: Check the updated scarb way to read profile specific values
            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        config.tokio_handle().block_on(events::execute(self, env_metadata, event_map))
    }
}

fn extract_events(manifest: &Manifest) -> HashMap<String, Vec<Event>> {
    fn inner_helper(events: &mut HashMap<String, Vec<Event>>, abi: abi::Contract) {
        for item in abi.into_iter() {
            if let Item::Event(e) = item {
                match e.kind {
                    abi::EventKind::Struct { .. } => {
                        let event_name = starknet_keccak(
                            e.name
                                .split("::")
                                .last()
                                .expect("valid fully qualified name")
                                .as_bytes(),
                        );
                        let vec = events.entry(event_name.to_string()).or_default();
                        vec.push(e.clone());
                    }
                    abi::EventKind::Enum { .. } => (),
                }
            }
        }
    }

    let mut events_map = HashMap::new();

    if let Some(abi) = manifest.world.abi.clone() {
        inner_helper(&mut events_map, abi);
    }

    if let Some(abi) = manifest.executor.abi.clone() {
        inner_helper(&mut events_map, abi);
    }

    for contract in &manifest.contracts {
        if let Some(abi) = contract.abi.clone() {
            inner_helper(&mut events_map, abi);
        }
    }

    for model in &manifest.contracts {
        if let Some(abi) = model.abi.clone() {
            inner_helper(&mut events_map, abi);
        }
    }

    events_map
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
