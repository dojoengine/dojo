use dojo::database::introspect::Introspect;

#[derive(Drop, Introspect)]
struct Base {
    value: u32,
}

#[derive(Drop, Introspect)]
struct WithArray {
    value: u32,
    arr: Array<u8>
}

#[derive(Drop, Introspect)]
struct WithArrayWithFixedSizeItem {
    value: u32,
    arr: Array<Base>
}

#[derive(Drop, Introspect)]
struct WithArrayWithDynamicSizeItem {
    value: u32,
    arr: Array<WithArray>
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

#[test]
fn test_base() {
    let size = Introspect::<Base>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 1);
}