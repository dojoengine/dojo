/// Compute the poseidon hash of a serialized ByteArray
fn hash(data: @ByteArray) -> felt252 {
    let mut serialized = ArrayTrait::new();
    Serde::serialize(data, ref serialized);
    poseidon::poseidon_hash_span(serialized.span())
}

/// Computes the entity id from the keys.
///
/// # Arguments
///
/// * `keys` - The keys of the entity.
///
/// # Returns
///
/// The entity id.
fn entity_id_from_keys(keys: Span<felt252>) -> felt252 {
    poseidon::poseidon_hash_span(keys)
}
