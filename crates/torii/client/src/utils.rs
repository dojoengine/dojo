use starknet::macros::short_string;
use starknet_crypto::{poseidon_hash_many, FieldElement};

/// Compute the base storage address for a given component of an entity.
pub fn compute_storage_base_address(
    model: FieldElement,
    entity_keys: &[FieldElement],
) -> FieldElement {
    poseidon_hash_many(&[short_string!("dojo_storage"), model, poseidon_hash_many(entity_keys)])
}

/// Compute all the storage addresses that are used for a given component of an entity when it is
/// stored in the World storage.
pub(crate) fn compute_all_storage_addresses(
    model: FieldElement,
    entity_keys: &[FieldElement],
    packed_size: u32,
) -> Vec<FieldElement> {
    let base = compute_storage_base_address(model, entity_keys);
    (0..packed_size).map(|i| base + i.into()).collect::<Vec<_>>()
}
