use std::collections::{HashMap, VecDeque};
use std::fs;

use anyhow::{anyhow, Context, Error, Result};
use cairo_lang_starknet::abi::{self, Event, EventKind, Item};
use cairo_lang_starknet::plugin::events::EventFieldKind;
use camino::Utf8PathBuf;
use dojo_lang::compiler::{DEPLOYMENTS_DIR, MANIFESTS_DIR};
use dojo_world::manifest::{DeployedManifest, ManifestMethods};
use dojo_world::metadata::Environment;
use starknet::core::types::{BlockId, EventFilter};
use starknet::core::utils::{parse_cairo_short_string, starknet_keccak};
use starknet::providers::Provider;

use crate::commands::events::EventsArgs;

pub async fn execute(
    args: EventsArgs,
    env_metadata: Option<Environment>,
    manifest_dir: &Utf8PathBuf,
) -> Result<()> {
    let EventsArgs {
        chunk_size,
        starknet,
        world,
        from_block,
        to_block,
        events,
        continuation_token,
        json,
        ..
    } = args;

    let provider = starknet.provider(env_metadata.as_ref())?;
    let chain_id = provider.chain_id().await?;
    let chain_id =
        parse_cairo_short_string(&chain_id).with_context(|| "Cannot parse chain_id as string")?;

    let events_map = if !json {
        let deployed_manifest = manifest_dir
            .join(MANIFESTS_DIR)
            .join(DEPLOYMENTS_DIR)
            .join(chain_id)
            .with_extension("toml");

        if !deployed_manifest.exists() {
            return Err(anyhow!("Run scarb migrate before running this command"));
        }

        Some(extract_events(&DeployedManifest::load_from_path(&deployed_manifest)?, manifest_dir)?)
    } else {
        None
    };

    let from_block = from_block.map(BlockId::Number);
    let to_block = to_block.map(BlockId::Number);
    // Currently dojo doesn't use custom keys for events. In future if custom keys are used this
    // needs to be updated for granular queries.
    let keys =
        events.map(|e| vec![e.iter().map(|event| starknet_keccak(event.as_bytes())).collect()]);

    let provider = starknet.provider(env_metadata.as_ref())?;
    let event_filter = EventFilter { from_block, to_block, address: world.world_address, keys };

    let res = provider.get_events(event_filter, continuation_token, chunk_size).await?;

    if let Some(events_map) = events_map {
        parse_and_print_events(res, events_map)?;
    } else {
        println!("{}", serde_json::to_string_pretty(&res)?);
    }

    Ok(())
}

fn parse_and_print_events(
    res: starknet::core::types::EventsPage,
    events_map: HashMap<String, Vec<Event>>,
) -> Result<()> {
    println!("Continuation token: {:?}", res.continuation_token);
    println!("----------------------------------------------");
    for event in res.events {
        if let Some(e) = parse_event(event.clone(), &events_map) {
            println!("{e}");
        } else {
            // Couldn't parse event
            println!("{}", serde_json::to_string_pretty(&event)?);
        }
    }
    Ok(())
}

fn parse_event(
    event: starknet::core::types::EmittedEvent,
    events_map: &HashMap<String, Vec<Event>>,
) -> Option<String> {
    let keys = event.keys;
    let event_hash = keys[0].to_string();
    let events = events_map.get(&event_hash)?;

    'outer: for e in events {
        let mut ret = format!("Event name: {}\n", e.name);
        let mut data = VecDeque::from(event.data.clone());

        // Length is two only when its custom event
        if keys.len() == 2 {
            let name = parse_cairo_short_string(&keys[1]).ok()?;
            ret.push_str(&format!("Model name: {}\n", name));
        }

        match &e.kind {
            EventKind::Struct { members } => {
                for field in members {
                    if field.kind != EventFieldKind::DataSerde {
                        continue;
                    }
                    match field.ty.as_str() {
                        "core::starknet::contract_address::ContractAddress"
                        | "core::starknet::class_hash::ClassHash" => {
                            let value = match data.pop_front() {
                                Some(addr) => addr,
                                None => continue 'outer,
                            };
                            ret.push_str(&format!("{}: {:#x}\n", field.name, value));
                        }
                        "core::felt252" => {
                            let value = match data.pop_front() {
                                Some(addr) => addr,
                                None => continue 'outer,
                            };
                            let value = match parse_cairo_short_string(&value) {
                                Ok(v) => v,
                                Err(_) => format!("{:#x}", value),
                            };
                            ret.push_str(&format!("{}: {}\n", field.name, value));
                        }
                        "core::integer::u8" => {
                            let value = match data.pop_front() {
                                Some(addr) => addr,
                                None => continue 'outer,
                            };
                            let num = match value.to_string().parse::<u8>() {
                                Ok(num) => num,
                                Err(_) => continue 'outer,
                            };

                            ret.push_str(&format!("{}: {}\n", field.name, num));
                        }
                        "dojo_examples::systems::move::Direction" => {
                            let value = match data.pop_front() {
                                Some(addr) => addr,
                                None => continue 'outer,
                            };
                            ret.push_str(&format!("{}: {}\n", field.name, value));
                        }
                        "core::array::Span::<core::felt252>" => {
                            let length = match data.pop_front() {
                                Some(addr) => addr,
                                None => continue 'outer,
                            };
                            let length = match length.to_string().parse::<usize>() {
                                Ok(len) => len,
                                Err(_) => continue 'outer,
                            };
                            ret.push_str(&format!("{}: ", field.name));
                            if data.len() >= length {
                                ret.push_str(&format!(
                                    "{:?}\n",
                                    data.drain(..length)
                                        .map(|e| format!("{:#x}", e))
                                        .collect::<Vec<_>>()
                                ));
                            } else {
                                continue 'outer;
                            }
                        }
                        _ => {
                            return None;
                        }
                    }
                }
                return Some(ret);
            }
            EventKind::Enum { .. } => unreachable!("shouldn't reach here"),
        }
    }

    None
}

fn extract_events(
    manifest: &DeployedManifest,
    manifest_dir: &Utf8PathBuf,
) -> Result<HashMap<String, Vec<Event>>, Error> {
    fn inner_helper(
        events: &mut HashMap<String, Vec<Event>>,
        abi_path: &Utf8PathBuf,
        manifest_dir: &Utf8PathBuf,
    ) -> Result<(), Error> {
        let full_abi_path = manifest_dir.join(abi_path);
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
    use camino::Utf8Path;
    use clap::Parser;
    use dojo_lang::compiler::{BASE_DIR, MANIFESTS_DIR};
    use dojo_world::manifest::BaseManifest;

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
        let manifest_dir = Utf8Path::new("../../examples/spawn-and-move").to_path_buf();
        let manifest =
            BaseManifest::load_from_path(&manifest_dir.join(MANIFESTS_DIR).join(BASE_DIR))
                .unwrap()
                .into();
        let result = extract_events(&manifest, &manifest_dir).unwrap();

        // we are just collection all events from manifest file so just verifying count should work
        assert!(result.len() == 2);
    }
}
