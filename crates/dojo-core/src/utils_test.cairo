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
    let namespace = dojo::model::Model::<MyModel>::namespace();
    let namespace_selector = dojo::model::Model::<MyModel>::namespace_selector();

    assert(dojo::utils::hash(@namespace) == namespace_selector, 'invalid computed hash');
}
