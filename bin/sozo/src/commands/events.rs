use std::collections::HashMap;
use std::fs;
use std::io::Error;

use anyhow::{anyhow, Result};
use cairo_lang_starknet::abi::{self, Event, Item};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use dojo_world::manifest::{DeployedManifest, ManifestMethods};
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

            Some(extract_events(
                &DeployedManifest::load_from_path(&manifest_path)?,
                &manifest_path,
            )?)
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

fn extract_events(
    manifest: &DeployedManifest,
    manifest_dir: &Utf8PathBuf,
) -> anyhow::Result<HashMap<String, Vec<Event>>> {
    fn inner_helper(
        events: &mut HashMap<String, Vec<Event>>,
        abi_path: &String,
        manifest_dir: &Utf8PathBuf,
    ) -> Result<(), Error> {
        let full_abi_path = manifest_dir.join(Utf8Path::new(abi_path));
        let abi: abi::Contract = serde_json::from_str(&fs::read_to_string(full_abi_path)?)?;

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

        Ok(())
    }

    let mut events_map = HashMap::new();

    if let Some(abi_path) = manifest.world.inner.abi() {
        inner_helper(&mut events_map, abi_path, manifest_dir)?;
    }

    if let Some(abi_path) = manifest.executor.inner.abi() {
        inner_helper(&mut events_map, abi_path, manifest_dir)?;
    }

    for contract in &manifest.contracts {
        if let Some(abi_path) = contract.inner.abi() {
            inner_helper(&mut events_map, abi_path, manifest_dir)?;
        }
    }

    for model in &manifest.contracts {
        if let Some(abi_path) = model.inner.abi() {
            inner_helper(&mut events_map, abi_path, manifest_dir)?;
        }
    }

    Ok(events_map)
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
        let manifest = DeployedManifest::load_from_path("./tests/test_data/manifest.json").unwrap();
        let result = extract_events(&manifest);

        // we are just collection all events from manifest file so just verifying count should work
        assert!(result.len() == 13);
    }
}
