use dojo::model::Model;
use dojo::utils::{bytearray_hash, selector_from_names};

#[derive(Drop, Copy, Serde)]
#[dojo::model(namespace: "my_namespace")]
struct MyModel {
    #[key]
    x: u8,
    y: u8
}

#[test]
fn test_hash_computation() {
    // Be sure that the namespace hash computed in `dojo-lang` in Rust is equal
    // to the one computed in Cairo by dojo::utils:hash
    let namespace = Model::<MyModel>::namespace();
    let namespace_hash = Model::<MyModel>::namespace_hash();

    assert(bytearray_hash(@namespace) == namespace_hash, 'invalid computed hash');
}

#[test]
fn test_selector_computation() {
    let namespace = Model::<MyModel>::namespace();
    let name = Model::<MyModel>::name();
    let selector = selector_from_names(@namespace, @name);
    assert(selector == Model::<MyModel>::selector(), 'invalid computed selector');
}
