use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockId, BlockWithTxs, MaybePendingStateUpdate};
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::macros::short_string;
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet::providers::Provider;
use starknet_crypto::{poseidon_hash_many, FieldElement};
use tracing::info;

use super::BlockProcessor;
use crate::state::State;

#[derive(Default)]
pub struct StateDiffProcessor {
    pub component: String,
    pub world: FieldElement,
    pub length: usize,
    pub keys: Vec<FieldElement>,
}

impl StateDiffProcessor {
    pub fn new(
        component: String,
        world: FieldElement,
        length: usize,
        keys: Vec<FieldElement>,
    ) -> Self {
        Self { component, world, length, keys }
    }
}

#[async_trait]
impl<S, T> BlockProcessor<S, T> for StateDiffProcessor
where
    S: State + Sync,
    T: Sync + Send + JsonRpcTransport + 'static,
{
    fn get_block_number(&self, block: &BlockWithTxs) -> String {
        block.block_number.to_string()
    }

    async fn process(
        &self,
        storage: &S,
        provider: &JsonRpcClient<T>,
        block: &BlockWithTxs,
    ) -> Result<(), Error> {
        info!("store set record: {}", self.component);
        // id is key for entity
        let id = poseidon_hash_many(&self.keys);
        // key is component's base storage key
        let key = poseidon_hash_many(&[
            short_string!("dojo_storage"),
            cairo_short_string_to_felt(&self.component).unwrap(),
            id,
        ]);

        // get State diff from JsonRpc
        let block_id = BlockId::Hash(block.block_hash);
        let maybe_state_update = provider.get_state_update(block_id).await?;
        let state_diff = match maybe_state_update {
            MaybePendingStateUpdate::Update(maybe_state_update) => maybe_state_update.state_diff,
            MaybePendingStateUpdate::PendingUpdate(maybe_state_update) => {
                maybe_state_update.state_diff
            }
        };

        let mut values = Vec::new();

        // loop from offset 0 to until it reaches length
        for i in 0..self.length {
            for storage_diff in state_diff.storage_diffs.iter() {
                if storage_diff.address == self.world {
                    for storage_entries in storage_diff.storage_entries.iter() {
                        if storage_entries.key == key + i.into() {
                            values.push(storage_entries.value);
                        }
                    }
                }
            }
        }

        storage.set_entity(self.component.clone(), self.keys.clone(), values).await?;
        Ok(())
    }
}
