use starknet::{
    core::types::{BlockId, EventFilter},
    core::utils::starknet_keccak,
    providers::Provider,
};
use toml::Value;

use crate::commands::events::EventsArgs;
use anyhow::Result;

pub async fn execute(args: EventsArgs, env_metadata: Option<Value>) -> Result<()> {
    let EventsArgs { chunk_size, starknet, world, from_block, to_block, events } = args;

    let from_block = from_block.map(|num| BlockId::Number(num));
    let to_block = to_block.map(|num| BlockId::Number(num));
    let keys = events.map(|e| {
        let mut ret = vec![];
        for event in e {
            ret.push(vec![starknet_keccak(event.as_bytes())]);
        }
        ret
    });

    let provider = starknet.provider(env_metadata.as_ref())?;
    let event_filter = EventFilter { from_block, to_block, address: world.world_address, keys };

    let res = provider.get_events(event_filter, None, chunk_size).await?;

    let value = serde_json::to_value(res)?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
