use starknet::{core::types::EventFilter, providers::Provider};
use toml::Value;

use crate::commands::events::EventsArgs;
use anyhow::Result;

pub async fn execute(args: EventsArgs, env_metadata: Option<Value>) -> Result<()> {
    let EventsArgs { chunk_size, starknet } = args;
    let provider = starknet.provider(env_metadata.as_ref())?;
    let event_filter = EventFilter { from_block: None, to_block: None, address: None, keys: None };
    let res = provider.get_events(event_filter, None, chunk_size).await;

    println!("{res:#?}");
    Ok(())
}
