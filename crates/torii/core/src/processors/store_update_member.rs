use anyhow::{Context, Error, Result};
use async_trait::async_trait;
use dojo_world::contracts::naming;
use dojo_world::contracts::world::WorldContractReader;
use num_traits::ToPrimitive;
use starknet::core::types::{Event, TransactionReceiptWithBlockInfo};
use starknet::core::utils::get_selector_from_name;
use starknet::providers::Provider;
use tracing::{info, warn};

use super::EventProcessor;
use crate::processors::{ENTITY_ID_INDEX, MODEL_INDEX};
use crate::sql::Sql;

pub(crate) const LOG_TARGET: &str = "torii_core::processors::store_update_member";

const MEMBER_INDEX: usize = 2;

#[derive(Default, Debug)]
pub struct StoreUpdateMemberProcessor;

#[async_trait]
impl<P> EventProcessor<P> for StoreUpdateMemberProcessor
where
    P: Provider + Send + Sync + std::fmt::Debug,
{
    fn event_key(&self) -> String {
        "StoreUpdateMember".to_string()
    }

    fn validate(&self, event: &Event) -> bool {
        if event.keys.len() > 1 {
            info!(
                target: LOG_TARGET,
                event_key = %<StoreUpdateMemberProcessor as EventProcessor<P>>::event_key(self),
                invalid_keys = %<StoreUpdateMemberProcessor as EventProcessor<P>>::event_keys_as_string(self, event),
                "Invalid event keys."
            );
            return false;
        }
        true
    }

    async fn process(
        &self,
        _world: &WorldContractReader<P>,
        db: &mut Sql,
        _block_number: u64,
        block_timestamp: u64,
        _transaction_receipt: &TransactionReceiptWithBlockInfo,
        event_id: &str,
        event: &Event,
    ) -> Result<(), Error> {
        let selector = event.data[MODEL_INDEX];
        let entity_id = event.data[ENTITY_ID_INDEX];
        let member_selector = event.data[MEMBER_INDEX];

        let model = db.model(selector).await?;
        let schema = model.schema;

        let mut member = schema
            .as_struct()
            .expect("model schema must be a struct")
            .children
            .iter()
            .find(|c| {
                get_selector_from_name(&c.name).expect("invalid selector for member name")
                    == member_selector
            })
            .context("member not found")?
            .clone();

        info!(
            target: LOG_TARGET,
            name = %model.name,
            entity_id = format!("{:#x}", entity_id),
            member = %member.name,
            "Store update member.",
        );

        let values_start = MEMBER_INDEX + 1;
        let values_end: usize =
            values_start + event.data[values_start].to_usize().context("invalid usize")?;

        // Skip the length to only get the values as they will be deserialized.
        let mut values = event.data[values_start + 1..=values_end].to_vec();

        let tag = naming::get_tag(&model.namespace, &model.name);

        if !db.does_entity_exist(tag.clone(), entity_id).await? {
            warn!(
                target: LOG_TARGET,
                tag,
                entity_id = format!("{:#x}", entity_id),
                "Entity not found, must be set before updating a member.",
            );

            return Ok(());
        }

        member.ty.deserialize(&mut values)?;

        db.set_model_member(&schema.name(), entity_id, false, &member, event_id, block_timestamp)
            .await?;
        Ok(())
    }
}
