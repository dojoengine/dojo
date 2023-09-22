use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::manifest::Model;
use starknet::core::types::{BlockId, BlockTag, BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use torii_client::contract::world::WorldContractReader;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct RegisterModelProcessor;

#[async_trait]
impl<P: Provider + Sync + 'static> EventProcessor<P> for RegisterModelProcessor {
    fn event_key(&self) -> String {
        "ComponentRegistered".to_string()
    }

    async fn process(
        &self,
        world: &WorldContractReader<'_, P>,
        db: &Sql,
        _provider: &P,
        _block: &BlockWithTxs,
        _invoke_receipt: &InvokeTransactionReceipt,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[0])?;
        let model = world.component(&name, BlockId::Tag(BlockTag::Latest)).await?;
        let _schema = model.schema(BlockId::Tag(BlockTag::Latest)).await?;
        info!("registered model: {}", name);

        db.register_model(Model { name, class_hash: event.data[1], ..Default::default() }).await?;
        Ok(())
    }
}
