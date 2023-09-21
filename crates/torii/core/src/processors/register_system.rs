use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::manifest::System;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use tracing::info;

use super::EventProcessor;
use crate::State;

#[derive(Default)]
pub struct RegisterSystemProcessor;

#[async_trait]
impl<S: State + Sync, T: JsonRpcTransport> EventProcessor<S, T> for RegisterSystemProcessor {
    fn event_key(&self) -> String {
        "SystemRegistered".to_string()
    }

    async fn process(
        &self,
        storage: &S,
        _provider: &JsonRpcClient<T>,
        _block: &BlockWithTxs,
        _invoke_receipt: &InvokeTransactionReceipt,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[0])?;

        info!("registered system: {}", name);

        storage
            .register_system(System {
                name: name.into(),
                class_hash: event.data[1],
                ..System::default()
            })
            .await?;

        Ok(())
    }
}
