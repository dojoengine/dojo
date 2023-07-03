use anyhow::Result;
use starknet::core::types::{BlockId, EventFilter};
use starknet::core::utils::starknet_keccak;
use starknet::providers::Provider;
use toml::Value;

use crate::commands::events::EventsArgs;

pub async fn execute(args: EventsArgs, env_metadata: Option<Value>) -> Result<()> {
    let EventsArgs { chunk_size, starknet, world, from_block, to_block, events } = args;

    let from_block = from_block.map(BlockId::Number);
    let to_block = to_block.map(BlockId::Number);
    // Currently dojo doesn't use custom keys for events. In future if custom keys are used this
    // needs to be updated for granular queries.
    let keys =
        events.map(|e| vec![e.iter().map(|event| starknet_keccak(event.as_bytes())).collect()]);

    let provider = starknet.provider(env_metadata.as_ref())?;
    let event_filter = EventFilter { from_block, to_block, address: world.world_address, keys };

    let res = provider.get_events(event_filter, None, chunk_size).await?;

    let value = serde_json::to_value(res)?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
