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
        let target_dir = config.target_dir().path_existent().unwrap();
        let manifest_path = target_dir.join(config.profile().as_str()).join("manifest.json");

        if !manifest_path.exists() {
            return Err(anyhow!("Run scarb migrate before running this command"));
        }

        let manifest = Manifest::load_from_path(manifest_path)?;
        let events = extract_events(&manifest);
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

            // TODO: Check the updated scarb way to read profile specific values
            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };
        config.tokio_handle().block_on(events::execute(self, env_metadata, events))
    }
}

fn extract_events(manifest: &Manifest) -> HashMap<String, Vec<Event>> {
    fn inner_helper(events: &mut HashMap<String, Vec<Event>>, contract: &Option<abi::Contract>) {
        if let Some(contract) = contract {
            for item in &contract.items {
                if let Item::Event(e) = item {
                    match e.kind {
                        abi::EventKind::Struct { .. } => {
                            let event_name =
                                starknet_keccak(e.name.split("::").last().unwrap().as_bytes());
                            let vec = events.entry(event_name.to_string()).or_insert(Vec::new());
                            vec.push(e.clone());
                        }
                        abi::EventKind::Enum { .. } => (),
                    }
                }
            }
        }
    }

    let mut events_map = HashMap::new();

    inner_helper(&mut events_map, &manifest.world.abi);
    inner_helper(&mut events_map, &manifest.executor.abi);

    for contract in &manifest.contracts {
        inner_helper(&mut events_map, &contract.abi);
    }

    for model in &manifest.models {
        inner_helper(&mut events_map, &model.abi);
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
