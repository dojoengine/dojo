use std::error::Error;

use async_trait::async_trait;
use starknet::macros::short_string;
use starknet_crypto::{poseidon_hash_many, FieldElement};

// TODO: is this low level enough?
/// Low level storage interface
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait EntityStorage {
    type Error: Error + Send + Sync;

    /// This function mimic `world::set_entity` of `dojo-core`
    async fn set(
        &mut self,
        component: FieldElement,
        keys: Vec<FieldElement>,
        values: Vec<FieldElement>,
    ) -> Result<(), Self::Error>;

    /// This function mimic `world::entity` of `dojo-core`
    async fn get(
        &self,
        component: FieldElement,
        keys: Vec<FieldElement>,
        length: usize,
    ) -> Result<Vec<FieldElement>, Self::Error>;
}

pub fn component_storage_base_address(
    component: FieldElement,
    keys: &[FieldElement],
) -> FieldElement {
    let id = poseidon_hash_many(keys);
    poseidon_hash_many(&[short_string!("dojo_storage"), component, id])
}
