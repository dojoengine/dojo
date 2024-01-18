use dojo::database::introspect::Introspect;

#[derive(Drop, Introspect)]
struct Base {
    value: u32,
}

#[derive(Drop, Introspect)]
struct Generic<T> {
    value: T,
}

#[derive(Drop, Introspect)]
struct FeltsArray {
    #[capacity(10)]
    felts: Array<felt252>,
}

#[test]
#[available_gas(2000000)]
fn test_generic_introspect() {
    let generic = Generic { value: Base { value: 123 } };
}
