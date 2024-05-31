/// Compute the poseidon hash of a serialized ByteArray
fn hash(data: @ByteArray) -> felt252 {
    let mut serialized = ArrayTrait::new();
    Serde::serialize(data, ref serialized);
    poseidon::poseidon_hash_span(serialized.span())
}
