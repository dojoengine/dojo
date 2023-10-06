use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::manifest::System;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use torii_client::contract::world::WorldContractReader;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct RegisterSystemProcessor;

#[async_trait]
impl<P: Provider + Sync> EventProcessor<P> for RegisterSystemProcessor {
    fn event_key(&self) -> String {
        "SystemRegistered".to_string()
    }

    async fn process(
        &self,
        _world: &WorldContractReader<'_, P>,
        db: &mut Sql,
        _provider: &P,
        _block: &BlockWithTxs,
        _invoke_receipt: &InvokeTransactionReceipt,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[0])?;

        info!("registered system: {}", name);

        db.register_system(System {
            name: name.into(),
            class_hash: event.data[1],
            ..System::default()
        })
        .await?;

        Ok(())
    }
}
