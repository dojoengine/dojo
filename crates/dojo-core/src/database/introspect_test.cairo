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
struct WithByteArray {
    value: u32,
    arr: ByteArray
}

#[derive(Drop, Introspect)]
struct WithTuple {
    value: u32,
    arr: (u8, u16, u32)
}

#[derive(Drop, Introspect)]
struct WithNestedTuple {
    value: u32,
    arr: (u8, (u16, u128, u256), u32)
}

#[derive(Drop, Introspect)]
struct WithNestedArrayInTuple {
    value: u32,
    arr: (u8, (u16, Array<u128>, u256), u32)
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
fn test_size_basic_struct() {
    let size = Introspect::<Base>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 1);
}

#[test]
fn test_size_with_array() {
    assert!(Introspect::<WithArray>::size().is_none());
}

#[test]
fn test_size_with_byte_array() {
    assert!(Introspect::<WithByteArray>::size().is_none());
}

#[test]
fn test_size_with_tuple() {
    let size = Introspect::<WithTuple>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 4);
}

#[test]
fn test_size_with_nested_tuple() {
    let size = Introspect::<WithNestedTuple>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 7);
}

#[test]
fn test_size_with_nested_array_in_tuple() {
    let size = Introspect::<WithNestedArrayInTuple>::size();
    assert!(size.is_none());
}