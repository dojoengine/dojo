use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::manifest::Component;
use starknet::core::types::{BlockWithTxs, Event, TransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tracing::info;

use super::EventProcessor;
use crate::state::State;

#[derive(Default)]
pub struct ComponentRegistrationProcessor;

#[async_trait]
impl<S: State + Sync, T: JsonRpcTransport> EventProcessor<S, T> for ComponentRegistrationProcessor {
    fn event_key(&self) -> String {
        "ComponentRegistered".to_string()
    }

    async fn process(
        &self,
        storage: &S,
        _provider: &JsonRpcClient<T>,
        _block: &BlockWithTxs,
        _transaction_receipt: &TransactionReceipt,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[0])?;

        info!("registered component: {}", name);

        storage
            .register_component(Component { name, class_hash: event.data[1], ..Default::default() })
            .await?;
        Ok(())
    }
}
