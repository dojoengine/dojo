use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use colored::Colorize;
use dojo_world::contracts::abigen::world::{self, Event as WorldEvent};
use dojo_world::diff::WorldDiff;
use scarb::core::Config;
use sozo_ops::model;
use sozo_scarbext::WorkspaceExt;
use starknet::core::types::{BlockId, BlockTag, EventFilter, Felt};
use starknet::core::utils::starknet_keccak;
use starknet::providers::Provider;

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
            let profile_config = ws.load_profile_config()?;

            let (world_diff, provider, _) =
                utils::get_world_diff_and_provider(self.starknet, self.world, &ws).await?;

            let provider = Arc::new(provider);

            let from_block = if let Some(world_block) =
                profile_config.env.as_ref().and_then(|e| e.world_block)
            {
                Some(BlockId::Number(world_block))
            } else {
                self.from_block.map(BlockId::Number)
            };

            let to_block = self.to_block.map(BlockId::Number);
            let keys = self
                .events
                .map(|e| vec![e.iter().map(|event| starknet_keccak(event.as_bytes())).collect()]);

            let event_filter = EventFilter {
                from_block,
                to_block,
                address: Some(world_diff.world_info.address),
                keys,
            };

            let res =
                provider.get_events(event_filter, self.continuation_token, self.chunk_size).await?;

            for event in &res.events {
                match world::Event::try_from(event) {
                    Ok(ev) => {
                        match_event(
                            &ev,
                            &world_diff,
                            event.block_number,
                            event.transaction_hash,
                            &provider,
                        )
                        .await
                        .unwrap_or_else(|e| {
                            tracing::error!(?e, "Failed to process event: {:?}", ev);
                        });
                    }
                    Err(e) => {
                        tracing::error!(
                            ?e,
                            "Failed to parse remote world event which is supposed to be valid."
                        );
                    }
                }
            }

            if let Some(continuation_token) = res.continuation_token {
                println!("Continuation token: {:?}", continuation_token);
                println!("----------------------------------------------");
            }

            Ok(())
        })
    }
}

/// Matches the event and prints it's content.
async fn match_event<P: Provider + Send + Sync>(
    event: &WorldEvent,
    world_diff: &WorldDiff,
    block_number: Option<u64>,
    transaction_hash: Felt,
    provider: P,
) -> Result<()> {
    // Get a mapping of all the known selectors and their addresses.
    let contract_addresses_from_selector = world_diff.get_contracts_addresses();
    // Do a reverse mapping to retrieve a contract selector from it's address.
    let contract_selectors_from_address: HashMap<Felt, Felt> =
        contract_addresses_from_selector.into_iter().map(|(s, a)| (a, s)).collect();
    // Finally, cache all the known tags by creating them once for each selector.
    let mut tags = HashMap::new();
    for (s, r) in world_diff.resources.iter() {
        tags.insert(s, r.tag());
    }

    let block_id = if let Some(block_number) = block_number {
        BlockId::Number(block_number)
    } else {
        BlockId::Tag(BlockTag::Pending)
    };

    let (name, content) = match event {
        WorldEvent::WorldSpawned(e) => (
            "World spawned".to_string(),
            format!("Creator address: {:?}\nWorld class hash: {:#066x}", e.creator, e.class_hash.0),
        ),
        WorldEvent::WorldUpgraded(e) => {
            ("World upgraded".to_string(), format!("World class hash: {:#066x}", e.class_hash.0))
        }
        WorldEvent::NamespaceRegistered(e) => {
            ("Namespace registered".to_string(), format!("Namespace: {}", e.namespace.to_string()?))
        }
        WorldEvent::ModelRegistered(e) => (
            "Model registered".to_string(),
            format!(
                "Namespace: {}\nName: {}\nClass hash: {:#066x}\nAddress: {:#066x}",
                e.namespace.to_string()?,
                e.name.to_string()?,
                e.class_hash.0,
                e.address.0
            ),
        ),
        WorldEvent::EventRegistered(e) => (
            "Event registered".to_string(),
            format!(
                "Namespace: {}\nName: {}\nClass hash: {:#066x}\nAddress: {:#066x}",
                e.namespace.to_string()?,
                e.name.to_string()?,
                e.class_hash.0,
                e.address.0
            ),
        ),
        WorldEvent::ContractRegistered(e) => (
            "Contract registered".to_string(),
            format!(
                "Namespace: {}\nName: {}\nClass hash: {:#066x}\nAddress: {:#066x}\nSalt: {:#066x}",
                e.namespace.to_string()?,
                e.name.to_string()?,
                e.class_hash.0,
                e.address.0,
                e.salt
            ),
        ),
        WorldEvent::ModelUpgraded(e) => {
            let tag = tags.get(&e.selector).unwrap();
            (
                format!("Model upgraded ({})", tag),
                format!(
                    "Selector: {:#066x}\nClass hash: {:#066x}\nAddress: {:#066x}\nPrev address: \
                     {:#066x}",
                    e.selector, e.class_hash.0, e.address.0, e.prev_address.0
                ),
            )
        }
        WorldEvent::EventUpgraded(e) => {
            let tag = tags.get(&e.selector).unwrap();
            (
                format!("Event upgraded ({})", tag),
                format!(
                    "Selector: {:#066x}\nClass hash: {:#066x}\nAddress: {:#066x}\nPrev address: \
                     {:#066x}",
                    e.selector, e.class_hash.0, e.address.0, e.prev_address.0
                ),
            )
        }
        WorldEvent::ContractUpgraded(e) => {
            let tag = tags.get(&e.selector).unwrap();
            (
                format!("Contract upgraded ({})", tag),
                format!("Selector: {:#066x}\nClass hash: {:#066x}", e.selector, e.class_hash.0,),
            )
        }
        WorldEvent::ContractInitialized(e) => {
            let tag = tags.get(&e.selector).unwrap();
            (
                format!("Contract initialized ({})", tag),
                format!(
                    "Selector: {:#066x}\nInit calldata: {}",
                    e.selector,
                    e.init_calldata
                        .iter()
                        .map(|f| format!("{:#066x}", f))
                        .collect::<Vec<String>>()
                        .join(", ")
                ),
            )
        }
        WorldEvent::WriterUpdated(e) => {
            let tag = tags.get(&e.resource).unwrap();
            let grantee =
                if let Some(selector) = contract_selectors_from_address.get(&e.contract.into()) {
                    tags.get(selector).unwrap().to_string()
                } else {
                    format!("{:#066x}", e.contract.0)
                };

            (
                "Writer updated".to_string(),
                format!("Target resource: {}\nContract: {}\nValue: {}", tag, grantee, e.value),
            )
        }
        WorldEvent::OwnerUpdated(e) => {
            let tag = tags.get(&e.resource).unwrap();
            let grantee =
                if let Some(selector) = contract_selectors_from_address.get(&e.contract.into()) {
                    tags.get(selector).unwrap().to_string()
                } else {
                    format!("{:#066x}", e.contract.0)
                };

            (
                "Owner updated".to_string(),
                format!("Target resource: {}\nContract: {}\nValue: {}", tag, grantee, e.value),
            )
        }
        WorldEvent::StoreSetRecord(e) => {
            let tag = tags.get(&e.selector).unwrap();
            let (record, _, _) = model::model_get(
                tag.clone(),
                e.keys.clone(),
                world_diff.world_info.address,
                provider,
                block_id,
            )
            .await?;

            (
                format!("Store set record ({})", tag),
                format!(
                    "Selector: {:#066x}\nEntity ID: {:#066x}\nKeys: {}\nValues: {}\nData:\n{}",
                    e.selector,
                    e.entity_id,
                    e.keys
                        .iter()
                        .map(|k| format!("{:#066x}", k))
                        .collect::<Vec<String>>()
                        .join(", "),
                    e.values
                        .iter()
                        .map(|v| format!("{:#066x}", v))
                        .collect::<Vec<String>>()
                        .join(", "),
                    record
                ),
            )
        }
        WorldEvent::StoreUpdateRecord(e) => {
            let tag = tags.get(&e.selector).unwrap();
            // TODO: model value impl + print.
            (
                format!("Store update record ({})", tag),
                format!(
                    "Selector: {:#066x}\nEntity ID: {:#066x}\nValues: {}",
                    e.selector,
                    e.entity_id,
                    e.values
                        .iter()
                        .map(|v| format!("{:#066x}", v))
                        .collect::<Vec<String>>()
                        .join(", "),
                ),
            )
        }
        WorldEvent::StoreUpdateMember(e) => {
            let tag = tags.get(&e.selector).unwrap();
            // TODO: pretty print of the value.
            (
                format!("Store update member ({})", tag),
                format!(
                    "Selector: {:#066x}\nEntity ID: {:#066x}\nMember selector: {:#066x}\nValues: \
                     {}",
                    e.selector,
                    e.entity_id,
                    e.member_selector,
                    e.values
                        .iter()
                        .map(|v| format!("{:#066x}", v))
                        .collect::<Vec<String>>()
                        .join(", "),
                ),
            )
        }
        WorldEvent::StoreDelRecord(e) => {
            let tag = tags.get(&e.selector).unwrap();
            (
                format!("Store del record ({})", tag),
                format!("Selector: {:#066x}\nEntity ID: {:#066x}", e.selector, e.entity_id,),
            )
        }
        WorldEvent::EventEmitted(e) => {
            let tag = tags.get(&e.selector).unwrap();
            let contract_tag = if let Some(selector) =
                contract_selectors_from_address.get(&e.system_address.into())
            {
                tags.get(selector).unwrap().to_string()
            } else {
                format!("{:#066x}", e.system_address.0)
            };

            // TODO: for events, we need to pull the schema and print the values accordingly.

            (
                format!("Event emitted ({})", tag),
                format!(
                    "Selector: {:#066x}\nContract: {}\nKeys: {}\nValues: {}",
                    e.selector,
                    contract_tag,
                    e.keys
                        .iter()
                        .map(|k| format!("{:#066x}", k))
                        .collect::<Vec<String>>()
                        .join(", "),
                    e.values
                        .iter()
                        .map(|v| format!("{:#066x}", v))
                        .collect::<Vec<String>>()
                        .join(", "),
                ),
            )
        }
        _ => ("Unprocessed event".to_string(), format!("Event: {:?}", event)),
    };

    let block_str = block_number.map(|n| n.to_string()).unwrap_or("pending".to_string());
    let ptr = format!("[block:{} / tx:{:#066x}]", block_str, transaction_hash).bright_black();

    println!("> {name} {ptr}\n{content}\n-----\n");

    Ok(())
}
