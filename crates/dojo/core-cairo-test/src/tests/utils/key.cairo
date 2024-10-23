use dojo::utils::{entity_id_from_keys, combine_key};

#[test]
fn test_entity_id_from_keys() {
    let keys = [1, 2, 3].span();
    assert(entity_id_from_keys(keys) == core::poseidon::poseidon_hash_span(keys), 'bad entity ID');
}

#[test]
fn test_combine_key() {
    assert(
        combine_key(1, 2) == core::poseidon::poseidon_hash_span([1, 2].span()), 'combine key error'
    );
}
