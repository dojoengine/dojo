use core::poseidon::poseidon_hash_span;

#[test]
fn test_bytearray_hash() {
    let bytes: ByteArray = "foo";
    let hash = bytearray_hash!("foo");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_eq!(computed, hash);
}

#[test]
fn test_bytearray_hash_empty() {
    let bytes: ByteArray = "";
    let hash = bytearray_hash!("");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_eq!(computed, hash);
}

#[test]
fn test_bytearray_hash_31() {
    let bytes: ByteArray = "0123456789012345678901234567890";
    let hash = bytearray_hash!("0123456789012345678901234567890");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_eq!(computed, hash);
}

#[test]
fn test_bytearray_hash_long() {
    let bytes: ByteArray = "0123456789012345678901234567890foo";
    let hash = bytearray_hash!("0123456789012345678901234567890foo");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_eq!(computed, hash);
}

#[test]
fn test_bytearray_hash_ne() {
    let bytes: ByteArray = "foo";
    let hash = bytearray_hash!("bar");
    let mut array = array![];
    bytes.serialize(ref array);
    let computed = poseidon_hash_span(array.span());
    assert_ne!(computed, hash);
}
