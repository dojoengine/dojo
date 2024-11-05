//! World's resources.

use starknet::ContractAddress;

/// Resource is the type of the resource that can be registered in the world.
///
/// Caching the namespace hash of a contract and model in the world saves gas, instead
/// of re-computing the descriptor each time, which involves several poseidon hash
/// operations.
///
/// Namespaced resources: Those resources are scoped by a namespace, which
/// defines a logical separation of resources. Namespaced resources are Model, Event and Contract.
///
/// - World: The world itself, identified by the selector 0.
///
/// - Namespace: ByteArray
/// Namespace is a unique resource type, identified by a `ByteArray`, to scope models, events and
/// contracts.
/// The poseidon hash of the serialized `ByteArray` is used as the namespace hash.
///
/// - Model: (ContractAddress, NamespaceHash)
/// A model defines data that can be stored in the world's storage.
///
/// - Event: (ContractAddress, NamespaceHash)
/// An event is never stored in the world's storage, but it's emitted by the world to be consumed by
/// off-chain components.
///
/// - Contract: (ContractAddress, NamespaceHash)
/// A contract defines user logic to interact with the world's data (models) and to emit events.
///
/// - Unregistered: The unregistered state, required to ensure the security of the world
/// to not have operations done on non-existent resources.
#[derive(Drop, starknet::Store, Serde, Default, Debug)]
pub enum Resource {
    Model: (ContractAddress, felt252),
    Event: (ContractAddress, felt252),
    Contract: (ContractAddress, felt252),
    Namespace: ByteArray,
    World,
    #[default]
    Unregistered,
}

#[generate_trait]
pub impl ResourceIsNoneImpl of ResourceIsNoneTrait {
    /// Returns true if the resource is unregistered, false otherwise.
    fn is_unregistered(self: @Resource) -> bool {
        match self {
            Resource::Unregistered => true,
            _ => false
        }
    }
}
