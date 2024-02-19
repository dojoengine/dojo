use std::collections::HashMap;

use anyhow::{anyhow, Result};
use cainome::parser::tokens::Token;
use cainome::parser::AbiParser;
use cairo_lang_starknet::abi;
use clap::Parser;
use dojo_world::manifest::Manifest;
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::Config;
use serde_json;
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

            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        config.tokio_handle().block_on(events::execute(self, env_metadata, event_map))
    }
}

fn is_event(token: &Token) -> bool {
    match token {
        Token::Composite(composite) => composite.is_event,
        _ => false,
    }
}

fn extract_events(manifest: &Manifest) -> HashMap<String, Vec<Token>> {
    //println!("manifest {:?}", manifest.world.abi.clone().unwrap());

    // Helper function to process ABI and populate events_map
    fn process_abi(abi: &abi::Contract, events_map: &mut HashMap<String, Vec<Token>>) {
        match serde_json::to_string(abi) {
            Ok(abi_str) => match AbiParser::tokens_from_abi_string(&abi_str, &HashMap::new()) {
                Ok(tokens) => {
                    for token in tokens.structs {
                        if is_event(&token) {
                            //println!("°°°°°°°°°°°°");
                            //println!("Token Name: {:?}", token.type_name());
                            //println!("Token: {:?}", token);
                            //println!("°°°°°°°°°°°°");

                            let event_name = starknet_keccak(token.type_name().as_bytes());
                            //println!("Event Name: {} {}", event_name, token.type_name());

                            let vec = events_map.entry(event_name.to_string()).or_default();
                            vec.push(token.clone());
                        }
                    }
                }
                Err(e) => println!("Error parsing ABI: {}", e),
            },
            Err(e) => println!("Error serializing Contract to JSON: {}", e),
        }
    }

    let mut events_map = HashMap::new();

    // Iterate over all ABIs in the manifest and process them
    if let Some(abi) = manifest.world.abi.as_ref() {
        process_abi(abi, &mut events_map);
    }

    for contract in &manifest.contracts {
        if let Some(abi) = contract.abi.clone() {
            process_abi(&abi, &mut events_map);
        }
    }

    for model in &manifest.contracts {
        if let Some(abi) = model.abi.clone() {
            process_abi(&abi, &mut events_map);
        }
    }

    //println!("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
    //println!("Events Map 2: {:?}", events_map);
    //println!("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
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

    #[test]
    fn extract_events_work_as_expected() {
        let manifest = Manifest::load_from_path("./tests/test_data/manifest.json").unwrap();
        let result = extract_events(&manifest);

        // we are just collection all events from manifest file so just verifying count should work
        assert!(result.len() == 13);
    }
}
