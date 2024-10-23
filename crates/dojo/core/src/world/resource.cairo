//! World's resources.

use starknet::ContractAddress;

/// Resource is the type of the resource that can be registered in the world.
///
/// Caching the namespace hash of a contract and model in the world saves gas, instead
/// of re-computing the descriptor each time, which involves several poseidon hash
/// operations.
///
/// - Model: (ContractAddress, NamespaceHash)
/// - Contract: (ContractAddress, NamespaceHash)
/// - Namespace: ByteArray
/// - World: The world itself, identified by the selector 0.
/// - Unregistered: The unregistered state.
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
    fn is_unregistered(self: @Resource) -> bool {
        match self {
            Resource::Unregistered => true,
            _ => false
        }
    }
}
