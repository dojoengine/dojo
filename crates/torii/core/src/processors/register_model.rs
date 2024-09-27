use std::collections::HashMap;

use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use cainome::cairo_serde::{ByteArray, CairoSerde};
use dojo_world::contracts::model::ModelReader;
use dojo_world::contracts::world::WorldContractReader;
use starknet::core::types::Event;
use starknet::providers::Provider;
use tracing::{debug, info};

use super::EventProcessor;
use crate::sql::utils::I256;
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::register_model";

#[derive(Default, Debug)]
pub struct RegisterModelProcessor;

#[async_trait]
impl<P> EventProcessor<P> for RegisterModelProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "ModelRegistered".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                target: LOG_TARGET,
                event_key = %<RegisterModelProcessor as EventProcessor<P>>::event_key(self),
                invalid_keys = %<RegisterModelProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
                "Invalid event keys."
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        world: &WorldContractReader<P>,
        db: &mut Sql,
        _cache: Option<&mut HashMap<String, I256>>,
        _block_number: u64,
        block_timestamp: u64,
        _event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let name = ByteArray::cairo_deserialize(&event.data, 0)?;
        let mut offset = ByteArray::cairo_serialized_size(&name);
        let namespace = ByteArray::cairo_deserialize(&event.data, offset)?;
        offset += ByteArray::cairo_serialized_size(&namespace);

        let name = name.to_string()?;
        let namespace = namespace.to_string()?;

        let model = world.model_reader(&namespace, &name).await?;
        let schema = model.schema().await?;
        let layout = model.layout().await?;

        let unpacked_size: u32 = model.unpacked_size().await?;
        let packed_size: u32 = model.packed_size().await?;

        let class_hash = event.data[offset];
        let contract_address = event.data[offset + 1];

        info!(
            target: LOG_TARGET,
            name = %name,
            "Registered model."
        );
        debug!(
            target: LOG_TARGET,
            name = %name,
            schema = ?schema,
            layout = ?layout,
            class_hash = ?class_hash,
            contract_address = ?contract_address,
            packed_size = %packed_size,
            unpacked_size = %unpacked_size,
            "Registered model content."
        );

        db.register_model(
            &namespace,
            schema,
            layout,
            class_hash,
            contract_address,
            packed_size,
            unpacked_size,
            block_timestamp,
        )
        .await?;

        Ok(())
    }
}
