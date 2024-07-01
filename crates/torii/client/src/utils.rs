use num_traits::FromPrimitive;
use starknet::core::types::Felt;
use starknet::macros::short_string;
use starknet_crypto::poseidon_hash_many;

/// Compute the base storage address for a given component of an entity.
pub fn compute_storage_base_address(model: Felt, entity_keys: &[Felt]) -> Felt {
    poseidon_hash_many(&[short_string!("dojo_storage"), model, poseidon_hash_many(entity_keys)])
}

/// Compute all the storage addresses that are used for a given component of an entity when it is
/// stored in the World storage.
pub(crate) fn compute_all_storage_addresses(
    model: Felt,
    entity_keys: &[Felt],
    packed_size: u32,
) -> Vec<Felt> {
    let base = compute_storage_base_address(model, entity_keys);
    (0..packed_size)
        .map(|i| base + Felt::from_u32(i).expect("u32 should fit in Felt"))
        .collect::<Vec<_>>()
}
