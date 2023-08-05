use crate::state::State;
use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use chrono::offset;
use dojo_types::component;
use starknet::core::types::BlockId;
use starknet::core::types::StateDiff;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::core::utils::parse_cairo_short_string;
use starknet::macros::short_string;
use starknet_crypto::poseidon_hash_many;
use starknet_crypto::FieldElement;
use tracing::info;

use super::StateDiffProcessor;

#[derive(Default)]
pub struct StoreSetRecordStateDiffProcessor;

#[async_trait]
impl<S: State + std::marker::Sync> StateDiffProcessor<S> for StoreSetRecordStateDiffProcessor {
    async fn process(
        &self,
        storage: &S,
        component: String,
        world: FieldElement,
        length: usize,
        keys: Vec<FieldElement>,
        state_diff: &StateDiff,
    ) -> Result<(), Error> {
        info!("store set record: {}", component);
        // id is key for entity
        let id = poseidon_hash_many(&keys);
        // key is component's base storage key
        let key = poseidon_hash_many(&[
            short_string!("dojo_storage"),
            cairo_short_string_to_felt(&component).unwrap(),
            id,
        ]);

        let mut values = Vec::new();

        // loop from offset 0 to until it reaches length
        for i in 0..length {
            for storage_diff in state_diff.storage_diffs.iter() {
                if storage_diff.address == world {
                    for storage_entries in storage_diff.storage_entries.iter() {
                        if storage_entries.key == key + i.into() {
                            values.push(storage_entries.value);
                        }
                    }
                }
            }
        }

        storage.set_entity(component, keys, values).await?;
        Ok(())
    }
}
