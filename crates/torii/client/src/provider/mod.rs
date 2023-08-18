use std::error::Error;

use async_trait::async_trait;
use starknet_crypto::FieldElement;

pub mod jsonrpc;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Provider {
    type Error: Error + Send + Sync;

    /// Get the class hash of a component.
    async fn component(&self, name: &str) -> Result<FieldElement, Self::Error>;

    /// Get the class hash of a system.
    async fn system(&self, name: &str) -> Result<FieldElement, Self::Error>;

    /// Get the component values of an entity.
    async fn entity(
        &self,
        component: &str,
        keys: Vec<FieldElement>,
    ) -> Result<Vec<FieldElement>, Self::Error>;
}
