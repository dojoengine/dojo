use core::poseidon::poseidon_hash_span;


#[test]
fn test_poseidon_hash_string() {
    let bytes: ByteArray = "foo";
    let hash = poseidon_hash_string!("foo");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_eq!(computed, hash);
}

#[test]
fn test_poseidon_hash_string_empty() {
    let bytes: ByteArray = "";
    let hash = poseidon_hash_string!("");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_eq!(computed, hash);
}

#[test]
fn test_poseidon_hash_string_31() {
    let bytes: ByteArray = "0123456789012345678901234567890";
    let hash = poseidon_hash_string!("0123456789012345678901234567890");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_eq!(computed, hash);
}

#[test]
fn test_poseidon_hash_string_long() {
    let bytes: ByteArray = "0123456789012345678901234567890foo";
    let hash = poseidon_hash_string!("0123456789012345678901234567890foo");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_eq!(computed, hash);
}

