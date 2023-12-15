use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::{BlockWithTxs, Event, InvokeTransactionReceipt};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use tracing::info;

use super::EventProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct RegisterModelProcessor;

#[async_trait]
impl<P> EventProcessor<P> for RegisterModelProcessor
where
    P: Provider + Send + Sync,
{
    fn event_key(&self) -> String {
        "ModelRegistered".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                "invalid keys for event {}: {}",
                <RegisterModelProcessor as EventProcessor<P>>::event_key(self),
                <RegisterModelProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        world: &WorldContractReader<P>,
        db: &mut Sql,
        _block: &BlockWithTxs,
        _invoke_receipt: &InvokeTransactionReceipt,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let name = parse_cairo_short_string(&event.data[0])?;

        let model = world.model_reader(&name).await?;
        let schema = model.schema().await?;
        let layout = model.layout().await?;

        let unpacked_size: u32 = model.unpacked_size().await?.try_into()?;
        let packed_size: u32 = model.packed_size().await?.try_into()?;

        info!("Registered model: {}", name);

        db.register_model(schema, layout, event.data[1], packed_size, unpacked_size).await?;

        Ok(())
    }
}
