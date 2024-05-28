use dojo::database::introspect::{Introspect, Layout, FieldLayout};

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

#[derive(Drop, IntrospectPacked)]
struct Vec3 {
    x: u32,
    y: u32,
    z: u32
}

#[derive(IntrospectPacked)]
struct Translation {
    from: Vec3,
    to: Vec3
}

#[derive(Drop, IntrospectPacked)]
struct StructInnerNotPacked {
    x: Base
}

#[derive(Drop, Introspect)]
enum EnumNoData {
    One,
    Two,
    Three
}

#[derive(Drop, Introspect)]
enum EnumWithSameData {
    One: u256,
    Two: u256,
    Three: u256
}

#[derive(Drop, Introspect)]
enum EnumWithSameTupleData {
    One: (u256, u32),
    Two: (u256, u32),
    Three: (u256, u32)
}

#[derive(Drop, Introspect)]
enum EnumWithVariousData {
    One: u32,
    Two: (u8, u16),
    Three: Array<u128>,
}


#[derive(Drop, IntrospectPacked)]
enum EnumPacked {
    A: u32,
    B: u32,
}

#[derive(Drop, IntrospectPacked)]
enum EnumInnerPacked {
    A: (EnumPacked, Vec3),
    B: (EnumPacked, Vec3),
}

#[derive(Drop, IntrospectPacked)]
enum EnumInnerNotPacked {
    A: (EnumPacked, Base),
    B: (EnumPacked, Base),
}

#[derive(Drop, Introspect)]
struct StructWithOption {
    x: Option<u16>
}

#[derive(Drop, Introspect)]
struct Generic<T> {
    value: T,
}

fn field(selector: felt252, layout: Layout) -> FieldLayout {
    FieldLayout { selector, layout }
}

fn fixed(values: Array<u8>) -> Layout {
    Layout::Fixed(values.span())
}

fn tuple(values: Array<Layout>) -> Layout {
    Layout::Tuple(values.span())
}

fn _enum(values: Array<Option<Layout>>) -> Layout {
    let mut items = array![];
    let mut i = 0;

    loop {
        if i >= values.len() {
            break;
        }

        let v = *values.at(i);
        match v {
            Option::Some(v) => { items.append(field(i.into(), v)); },
            Option::None => { items.append(field(i.into(), fixed(array![]))) }
        }

        i += 1;
    };

    Layout::Enum(items.span())
}

fn arr(item_layout: Layout) -> Layout {
    Layout::Array(array![item_layout].span())
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

#[test]
fn test_size_of_enum_without_variant_data() {
    let size = Introspect::<EnumNoData>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 1);
}

#[test]
fn test_size_of_enum_with_same_variant_data() {
    let size = Introspect::<EnumWithSameData>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 3);
}

#[test]
fn test_size_of_enum_with_same_tuple_variant_data() {
    let size = Introspect::<EnumWithSameTupleData>::size();
    assert!(size.is_some());
    assert!(size.unwrap() == 4);
}


#[test]
fn test_size_of_struct_with_option() {
    let size = Introspect::<StructWithOption>::size();
    assert!(size.is_none());
}

#[test]
fn test_size_of_enum_with_variant_data() {
    let size = Introspect::<EnumWithVariousData>::size();
    assert!(size.is_none());
}

#[test]
fn test_layout_of_enum_without_variant_data() {
    let layout = Introspect::<EnumNoData>::layout();
    let expected = _enum(array![ // One
    Option::None, // Two
     Option::None, // Three
     Option::None,]);

    assert!(layout == expected);
}

#[test]
fn test_layout_of_enum_with_variant_data() {
    let layout = Introspect::<EnumWithVariousData>::layout();
    let expected = _enum(
        array![
            // One
            Option::Some(fixed(array![32])),
            // Two
            Option::Some(tuple(array![fixed(array![8]), fixed(array![16])])),
            // Three
            Option::Some(arr(fixed(array![128]))),
        ]
    );

    assert!(layout == expected);
}

#[test]
fn test_layout_of_struct_with_option() {
    let layout = Introspect::<StructWithOption>::layout();
    let expected = Layout::Struct(
        array![field(selector!("x"), _enum(array![Option::Some(fixed(array![16])), Option::None]))]
            .span()
    );

    assert!(layout == expected);
}

#[test]
fn test_layout_of_packed_struct() {
    let layout = Introspect::<Vec3>::layout();
    let expected = Layout::Fixed(array![32, 32, 32].span());

    assert!(layout == expected);
}

#[test]
fn test_layout_of_inner_packed_struct() {
    let layout = Introspect::<Translation>::layout();
    let expected = Layout::Fixed(array![32, 32, 32, 32, 32, 32].span());

    assert!(layout == expected);
}

#[test]
#[should_panic(expected: ("A packed model layout must contain Fixed layouts only.",))]
fn test_layout_of_not_packed_inner_struct() {
    let _ = Introspect::<StructInnerNotPacked>::layout();
}


#[test]
fn test_layout_of_packed_enum() {
    let layout = Introspect::<EnumPacked>::layout();
    let expected = Layout::Fixed(array![8, 32].span());

    assert!(layout == expected);
}

#[test]
fn test_layout_of_inner_packed_enum() {
    let layout = Introspect::<EnumInnerPacked>::layout();
    let expected = Layout::Fixed(array![8, 8, 32, 32, 32, 32].span());

    assert!(layout == expected);
}

#[test]
#[should_panic(expected: ("A packed model layout must contain Fixed layouts only.",))]
fn test_layout_of_not_packed_inner_enum() {
    let _ = Introspect::<EnumInnerNotPacked>::layout();
}
