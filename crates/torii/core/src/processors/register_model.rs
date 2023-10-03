use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
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
        "ModelRegistered".to_string()
    }

    async fn process(
        &self,
        world: &WorldContractReader<'_, P>,
        db: &mut Sql,
        _provider: &P,
        _block: &BlockWithTxs,
        _invoke_receipt: &InvokeTransactionReceipt,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[0])?;

        // TODO: remove BlockId as argument
        let model = world.model(&name, BlockId::Tag(BlockTag::Latest)).await?;
        let schema = model.schema(BlockId::Tag(BlockTag::Latest)).await?;
        let layout = model.layout(BlockId::Tag(BlockTag::Latest)).await?;

        let unpacked_size: u8 =
            model.unpacked_size(BlockId::Tag(BlockTag::Latest)).await?.try_into()?;
        let packed_size: u8 =
            model.packed_size(BlockId::Tag(BlockTag::Latest)).await?.try_into()?;

        info!("Registered model: {}", name);

        db.register_model(schema, layout, event.data[1], packed_size, unpacked_size).await?;

        Ok(())
    }
}
