use dojo::utils::serialize_inline;

/// Computes the entity id from the keys.
///
/// # Arguments
///
/// * `keys` - The keys of the entity.
///
/// # Returns
///
/// The entity id.
pub fn entity_id_from_keys(keys: Span<felt252>) -> felt252 {
    core::poseidon::poseidon_hash_span(keys)
}

/// Combine parent and child keys to build one full key.
pub fn combine_key(parent_key: felt252, child_key: felt252) -> felt252 {
    core::poseidon::poseidon_hash_span([parent_key, child_key].span())
}

/// Computes the entity id from the key.
pub fn entity_id_from_key<K, +Serde<K>>(key: @K) -> felt252 {
    entity_id_from_keys(serialize_inline::<K>(key))
}
