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

#[derive(Drop, Introspect)]
enum EnumNoData {
    One,
    Two,
    Three
}

#[derive(Drop, Introspect)]
enum EnumWithData {
    One: u32,
    Two: (u8, u16),
    Three: Array<u128>,
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

fn item(layout: Layout) -> FieldLayout {
    FieldLayout { selector: '', layout }
}

fn fixed(values: Array<u8>) -> Layout {
    Layout::Fixed(values.span())
}

fn tuple(values: Array<Layout>) -> Layout {
    let mut items = array![];
    let mut i: u32 = 0;
    loop {
        if i >= values.len() { break; }
        let v = *values.at(i);

        items.append(item(v));

        i += 1;
    };

    Layout::Tuple(items.span())
}

fn _enum(values: Array<Option<Layout>>) -> Layout {
    let mut items = array![];
    let mut i = 0;

    loop {
        if i >= values.len() { break; }

        let v = *values.at(i);
        match v {
            Option::Some(v) => {
                items.append(
                    field(
                        i.into(),
                        tuple(
                            array![
                                fixed(array![8]),
                                v
                            ]
                        )
                    )
                );
            },
            Option::None => {
                items.append(
                    field(
                        i.into(),
                        fixed(array![8])
                    )
                )
            }
        }

        i += 1;
    };

    Layout::Enum(items.span())
}

fn arr(item_layout: Layout) -> Layout {
    Layout::Array(
        array![item(item_layout)].span()
    )
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
    assert!(size.is_none());
}

#[test]
fn test_size_of_struct_with_option() {
    let size = Introspect::<StructWithOption>::size();
    assert!(size.is_none());
}

#[test]
fn test_size_of_enum_with_variant_data() {
    let size = Introspect::<EnumWithData>::size();
    assert!(size.is_none());
}

#[test]
fn test_layout_of_enum_without_variant_data() {
    let layout = Introspect::<EnumNoData>::layout();
    let expected = Layout::Enum(
        array![
            // One
            field(0, tuple(array![fixed(array![8])])),
            // Two
            field(1, tuple(array![fixed(array![8])])),
            // Three
            field(2, tuple(array![fixed(array![8])])),
        ].span()
        );

    assert!(layout == expected);
}

#[test]
fn test_layout_of_enum_with_variant_data() {
    let layout = Introspect::<EnumWithData>::layout();
    let expected = Layout::Enum(
        array![
            // One
            field(
                0,
                tuple(
                    array![
                        fixed(array![8]),
                        fixed(array![32]),
                    ]
                )
            ),

            // Two
            field(
                1,
                tuple(
                    array![
                        fixed(array![8]),
                        tuple(
                            array![
                                fixed(array![8]),
                                fixed(array![16]),
                            ]
                        )
                    ]
                )
            ),

            // Three
            field(
                2,
                tuple(
                    array![
                        fixed(array![8]),
                        arr(fixed(array![128])),
                    ]
                )
            ),
        ].span()
        );

    assert!(layout == expected);
}

fn test_layout_of_struct_with_option() {
    let layout = Introspect::<StructWithOption>::layout();
    let expected = Layout::Struct(
        array![
            field(
                selector!("x"),
                _enum(
                    array![
                        Option::Some(fixed(array![16])),
                        Option::None
                    ]
                )
            )
        ].span()
    );

    assert!(layout == expected);
}