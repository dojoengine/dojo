use dojo::database::introspect::Introspect;

#[derive(Drop, Introspect)]
struct Base {
    value: u32,
}

#[derive(Drop, Introspect)]
struct Generic<T> {
    value: T,
}

#[test]
#[available_gas(2000000)]
fn test_generic_introspect() {
    let _generic = Generic { value: Base { value: 123 } };
}
