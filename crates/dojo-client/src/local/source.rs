use async_trait::async_trait;
use starknet::core::types::FieldElement;

use crate::local::storage::Storage;
use crate::provider::Provider;

/// Represents a source of which the state of a World can be loaded from.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Source: Provider {
    type Error;
    type Head;

    /// Load the state at of a World at `address` from the source and store it in `state`.
    ///
    /// Perform a one time load of the state at `address` in `state`. Suitable for doing a
    /// initial load of the state before syncing it with [`Self::sync()`].
    async fn load<S: Storage>(
        &self,
        address: FieldElement,
        state: &mut S,
    ) -> Result<(), <Self as Source>::Error>;

    /// Perform a sync operation to update the state at `address` in `state`.
    ///
    /// This can be a endless loop that keeps updating the state until it is synced
    /// with the source.
    ///
    /// NOTE: Maybe can put this in another trait that extends Source?
    async fn sync<S: Storage>(
        &self,
        address: FieldElement,
        state: &mut S,
    ) -> Result<(), <Self as Source>::Error>;

    /// Get the current `head` of the remote source to track
    /// how much the state has synced with source.
    fn head(&self) -> Result<Self::Head, <Self as Source>::Error>;

    /// Set the current head of the remote source.
    fn set_head(&mut self, head: Self::Head) -> Result<(), <Self as Source>::Error>;
}
