use dojo::meta::{FieldLayout, Layout};
use dojo::storage::layout::{
    *, delete_fixed_array_layout, read_fixed_array_layout, write_fixed_array_layout,
};
use dojo::storage::packing::PACKING_MAX_BITS;

#[test]
fn test_fixed_layout() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    let layout = [8, 16, 32].span();

    // first, read uninitialized data
    let mut read_data = array![];
    read_fixed_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0, 0, 0], "default fixed layout reading");

    // then, write and read back data
    let mut read_data = array![];
    let values = array![10, 20, 30];
    let mut offset = 0;

    write_fixed_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_fixed_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, values, "fixed layout writing/reading back");
    assert_eq!(offset, 3, "fixed layout writing/reading back: bad offset");

    // write and read back data with offset
    let mut read_data = array![];
    let values = array![1, 2, 3, 10, 20, 30];
    let mut offset = 3;

    write_fixed_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_fixed_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![10, 20, 30], "fixed layout writing/reading back (with offset)");
    assert_eq!(offset, 6, "fixed layout writing/reading back (with offset): bad offset");

    // delete written data and read back default values
    let mut read_data = array![];
    delete_fixed_layout(MODEL_KEY, KEY, layout);
    read_fixed_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0, 0, 0], "fixed layout deleting");
}

#[test]
fn test_tuple_layout() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // tuple: (u16, u32, u64)
    let layout = [
        Layout::Tuple(
            [Layout::Fixed([16].span()), Layout::Fixed([32].span()), Layout::Fixed([64].span())]
                .span(),
        )
    ]
        .span();

    // first, read uninitialized data
    let mut read_data = array![];
    read_tuple_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0, 0, 0], "default tuple layout reading");

    // then, write and read back data
    let mut read_data = array![];
    let values = array![10, 20, 30];
    let mut offset = 0;

    write_tuple_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_tuple_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, values, "tuple layout writing/reading back");
    assert_eq!(offset, 3, "tuple layout writing/reading back: bad offset");

    // then, write and read back data (with offset)
    let mut read_data = array![];
    let values = array![1, 2, 3, 10, 20, 30];
    let mut offset = 3;

    write_tuple_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_tuple_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![10, 20, 30], "tuple layout writing/reading back (with offset)");
    assert_eq!(offset, 6, "tuple layout writing/reading back (with offset): bad offset");

    // delete written data and read back default values
    let mut read_data = array![];
    delete_tuple_layout(MODEL_KEY, KEY, layout);
    read_tuple_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0, 0, 0], "tuple layout deleting");
}

#[test]
fn test_byte_array_layout() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // first, read uninitialized data
    let mut read_data = array![];
    read_byte_array_layout(MODEL_KEY, KEY, ref read_data);

    assert_eq!(read_data, array![0, 0, 0], "default byte array layout reading");

    // then, write and read back data
    let mut read_data = array![];
    let values = array![0, 0x68656c6c6f, 0x05]; // "hello"
    let mut offset = 0;

    write_byte_array_layout(MODEL_KEY, KEY, values.span(), ref offset);
    read_byte_array_layout(MODEL_KEY, KEY, ref read_data);

    assert_eq!(read_data, values, "byte array layout writing/reading back");
    assert_eq!(offset, 3, "byte array layout writing/reading back: bad offset");

    // then, write and read back data (with offset)
    let mut read_data = array![];
    let values = array![1, 2, 3, 0, 0x68656c6c6f, 0x05]; // "hello"
    let mut offset = 3;

    write_byte_array_layout(MODEL_KEY, KEY, values.span(), ref offset);
    read_byte_array_layout(MODEL_KEY, KEY, ref read_data);

    assert_eq!(
        read_data,
        array![0, 0x68656c6c6f, 0x05],
        "byte array layout writing/reading back (with offset)",
    );
    assert_eq!(offset, 6, "byte array layout writing/reading back (with offset): bad offset");

    // delete written data and read back default values
    let mut read_data = array![];
    delete_byte_array_layout(MODEL_KEY, KEY);
    read_byte_array_layout(MODEL_KEY, KEY, ref read_data);

    assert_eq!(read_data, array![0, 0, 0], "byte array layout deleting");
}

#[test]
#[should_panic(expected: ('invalid array length',))]
fn test_read_byte_array_layout_invalid_array_length() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // use fixed layout to write bad values
    let mut offset = 0;
    write_fixed_layout(
        MODEL_KEY, KEY, [4_294_967_295].span(), ref offset, [PACKING_MAX_BITS].span(),
    );

    let mut read_data = array![];
    read_byte_array_layout(MODEL_KEY, KEY, ref read_data);
}

#[test]
#[should_panic(expected: ('Invalid values length',))]
fn test_byte_array_layout_bad_input_length() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    let mut offset = 0;

    write_byte_array_layout(MODEL_KEY, KEY, [0].span(), ref offset);
}

#[test]
#[should_panic(expected: ('invalid array length',))]
fn test_byte_array_layout_bad_array_length_value() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    let mut offset = 0;

    write_byte_array_layout(MODEL_KEY, KEY, [4_294_967_295, 2, 3, 4].span(), ref offset);
}

#[test]
#[should_panic(expected: ('Invalid values length',))]
fn test_byte_array_layout_bad_array_length() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    let mut offset = 0;

    write_byte_array_layout(MODEL_KEY, KEY, [1, 2, 3].span(), ref offset);
}

#[test]
fn test_array_layout() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // Array<u32>
    let layout = [Layout::Fixed([32].span())].span();

    // first, read uninitialized data
    let mut read_data = array![];
    read_array_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0], "default array layout reading");

    // then, write and read back data
    let mut read_data = array![];
    let values = array![4, 1, 2, 3, 4];
    let mut offset = 0;

    write_array_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_array_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, values, "fixed layout writing/reading back");
    assert_eq!(offset, 5, "fixed layout writing/reading back: bad offset");

    // then, write and read back data (with offset)
    let mut read_data = array![];
    let values = array![1, 2, 3, 4, 1, 2, 3, 4];
    let mut offset = 3;

    write_array_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_array_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![4, 1, 2, 3, 4], "fixed layout writing/reading back (with offset)");
    assert_eq!(offset, 8, "fixed layout writing/reading back (with offset): bad offset");

    // delete written data and read back default values
    let mut read_data = array![];
    delete_array_layout(MODEL_KEY, KEY);
    read_array_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0], "fixed layout deleting");
}

#[test]
#[should_panic(expected: ('invalid array length',))]
fn test_read_array_layout_bad_array_length() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // Array<u32>
    let layout = [Layout::Fixed([32].span())].span();

    // use fixed layout to write bad values
    let mut offset = 0;
    write_fixed_layout(
        MODEL_KEY, KEY, [4_294_967_296].span(), ref offset, [PACKING_MAX_BITS].span(),
    );

    let mut read_data = array![];
    read_array_layout(MODEL_KEY, KEY, ref read_data, layout);
}

#[test]
#[should_panic(expected: ('Invalid values length',))]
fn test_array_layout_bad_values_length() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // Array<u32>
    let layout = [Layout::Fixed([32].span())].span();

    let mut offset = 2;
    write_array_layout(MODEL_KEY, KEY, [4, 1].span(), ref offset, layout);
}

#[test]
#[should_panic(expected: ('invalid array length',))]
fn test_array_layout_bad_array_length() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // Array<u32>
    let layout = [Layout::Fixed([32].span())].span();

    let mut offset = 0;
    write_array_layout(MODEL_KEY, KEY, [4_294_967_296, 1, 2, 3].span(), ref offset, layout);
}

#[test]
fn test_struct_layout() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // struct { x: u8, y: u32 }
    let layout = [
        FieldLayout { selector: selector!("x"), layout: Layout::Fixed([8].span()) },
        FieldLayout { selector: selector!("y"), layout: Layout::Fixed([32].span()) },
    ]
        .span();

    // first, read uninitialized data
    let mut read_data = array![];
    read_struct_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0, 0], "default struct layout reading");

    // then, write and read back data
    let mut read_data = array![];
    let values = array![10, 20];
    let mut offset = 0;

    write_struct_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_struct_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, values, "struct layout writing/reading back");
    assert_eq!(offset, 2, "struct layout writing/reading back: bad offset");

    // write and read back data with offset
    let mut read_data = array![];
    let values = array![1, 2, 3, 10, 20];
    let mut offset = 3;

    write_struct_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_struct_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![10, 20], "struct layout writing/reading back (with offset)");
    assert_eq!(offset, 5, "struct layout writing/reading back (with offset): bad offset");

    // delete written data and read back default values
    let mut read_data = array![];
    delete_struct_layout(MODEL_KEY, KEY, layout);
    read_struct_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0, 0], "struct layout deleting");
}

#[test]
fn test_enum_layout() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // enum E { X: u8, Y: u32 }
    let layout = [
        FieldLayout { selector: 1, layout: Layout::Fixed([8].span()) },
        FieldLayout { selector: 2, layout: Layout::Fixed([32].span()) },
    ]
        .span();

    // first, read uninitialized data
    let mut read_data = array![];
    read_enum_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0], "default enum layout reading");

    // then, write and read back data
    let mut read_data = array![];
    let values = array![2, 20];
    let mut offset = 0;

    write_enum_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_enum_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, values, "enum layout writing/reading back");
    assert_eq!(offset, 2, "enum layout writing/reading back: bad offset");

    // write and read back data with offset
    let mut read_data = array![];
    let values = array![1, 2, 3, 2, 20];
    let mut offset = 3;

    write_enum_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_enum_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![2, 20], "enum layout writing/reading back (with offset)");
    assert_eq!(offset, 5, "enum layout writing/reading back (with offset): bad offset");

    // delete written data and read back default values
    let mut read_data = array![];
    delete_enum_layout(MODEL_KEY, KEY, layout);
    read_enum_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0], "enum layout deleting");
}

#[test]
#[should_panic(expected: ('invalid variant value',))]
fn test_read_enum_layout_invalid_variant_value() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // enum E { X: u8, Y: u32 }
    let layout = [
        FieldLayout { selector: 1, layout: Layout::Fixed([8].span()) },
        FieldLayout { selector: 2, layout: Layout::Fixed([32].span()) },
    ]
        .span();

    // use fixed layout to write bad values
    let mut offset = 0;
    write_fixed_layout(MODEL_KEY, KEY, [256].span(), ref offset, [PACKING_MAX_BITS].span());

    let mut read_data = array![];
    read_enum_layout(MODEL_KEY, KEY, ref read_data, layout);
}

#[test]
#[should_panic(expected: "Unable to find the variant layout for variant 3")]
fn test_read_enum_layout_unexisting_variant_value() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // enum E { X: u8, Y: u32 }
    let layout = [
        FieldLayout { selector: 1, layout: Layout::Fixed([8].span()) },
        FieldLayout { selector: 2, layout: Layout::Fixed([32].span()) },
    ]
        .span();

    // use fixed layout to write bad values
    let mut offset = 0;
    write_fixed_layout(MODEL_KEY, KEY, [3].span(), ref offset, [PACKING_MAX_BITS].span());

    let mut read_data = array![];
    read_enum_layout(MODEL_KEY, KEY, ref read_data, layout);
}

#[test]
#[should_panic(expected: ('invalid variant value',))]
fn test_enum_layout_invalid_variant_value() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // enum E { X: u8, Y: u32 }
    let layout = [
        FieldLayout { selector: 1, layout: Layout::Fixed([8].span()) },
        FieldLayout { selector: 2, layout: Layout::Fixed([32].span()) },
    ]
        .span();

    let mut offset = 0;
    write_enum_layout(MODEL_KEY, KEY, [256].span(), ref offset, layout);
}

#[test]
#[should_panic(expected: "Unable to find the variant layout for variant 3")]
fn test_enum_layout_unexisting_variant() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // enum E { X: u8, Y: u32 }
    let layout = [
        FieldLayout { selector: 1, layout: Layout::Fixed([8].span()) },
        FieldLayout { selector: 2, layout: Layout::Fixed([32].span()) },
    ]
        .span();

    let mut offset = 0;
    write_enum_layout(MODEL_KEY, KEY, [3].span(), ref offset, layout);
}

#[test]
fn test_fixed_array_layout() {
    const MODEL_KEY: felt252 = 1;
    const KEY: felt252 = 2;

    // fixed array: [u8; 3]
    let layout = [(Layout::Fixed([8].span()), 3)].span();

    // first, read uninitialized data
    let mut read_data = array![];
    read_fixed_array_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0, 0, 0], "default fixed size array layout reading");

    // then, write and read back data
    let mut read_data = array![];
    let values = array![1, 2, 3];
    let mut offset = 0;

    write_fixed_array_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_fixed_array_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, values, "fixed size array layout writing/reading back");
    assert_eq!(offset, 3, "fixed size array layout writing/reading back: bad offset");

    // write and read back data with offset
    let mut read_data = array![];
    let values = array![1, 2, 3, 4, 5, 6, 7];
    let mut offset = 4;

    write_fixed_array_layout(MODEL_KEY, KEY, values.span(), ref offset, layout);
    read_fixed_array_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(
        read_data, array![5, 6, 7], "fixed size array layout writing/reading back (with offset)",
    );
    assert_eq!(offset, 7, "fixed size array layout writing/reading back (with offset): bad offset");

    // delete written data and read back default values
    let mut read_data = array![];
    delete_fixed_array_layout(MODEL_KEY, KEY, layout);
    read_fixed_array_layout(MODEL_KEY, KEY, ref read_data, layout);

    assert_eq!(read_data, array![0, 0, 0], "fixed size array layout deleting");
}
