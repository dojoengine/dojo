pub mod jsonrpc;

use async_trait::async_trait;
use starknet::core::types::FieldElement;

/// A [`Provider`] defines an interface for getting state of a World at [`Provider::Address`].
///
/// It is different with [`StorageReader`] in which a [`Provider`] may be a direct access to the
/// blockchain where the World contract is deployed or any where the World state is stored.
///
/// For what it is worth, a type that implements a [`StorageReader`] may also be a [`Provider`] as it
/// provide state access of a World.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait Provider {
    type Error;

    async fn get_executor(&self, world_address: FieldElement);

    async fn get_system(&self, world_address: FieldElement, class_hash: FieldElement);

    async fn get_system_hash(&self, world_address: FieldElement, name: String);

    async fn get_component(&self, world_address: FieldElement, class_hash: FieldElement);

    async fn get_component_hash(&self, world_address: FieldElement, name: String);
}
