use dojo::meta::{FieldLayout, Layout};
use dojo::utils::{combine_key, find_field_layout};
use super::{database, packing};

// the minimum internal size of an empty ByteArray
const MIN_BYTE_ARRAY_SIZE: u32 = 3;

// the maximum allowed index for an enum variant
const MAX_VARIANT_INDEX: u256 = 256;

/// Write values to the world storage.
///
/// # Arguments
/// * `model` - the model selector.
/// * `key` - the object key.
/// * `values` - the object values.
/// * `offset` - the start of object values in the `values` parameter.
/// * `layout` - the object values layout.
pub fn write_layout(
    model: felt252, key: felt252, values: Span<felt252>, ref offset: u32, layout: Layout,
) {
    match layout {
        Layout::Fixed(layout) => { write_fixed_layout(model, key, values, ref offset, layout); },
        Layout::Struct(layout) => { write_struct_layout(model, key, values, ref offset, layout); },
        Layout::Array(layout) => { write_array_layout(model, key, values, ref offset, layout); },
        Layout::FixedArray(layout) => {
            write_fixed_array_layout(model, key, values, ref offset, layout);
        },
        Layout::Tuple(layout) => { write_tuple_layout(model, key, values, ref offset, layout); },
        Layout::ByteArray => { write_byte_array_layout(model, key, values, ref offset); },
        Layout::Enum(layout) => { write_enum_layout(model, key, values, ref offset, layout); },
    }
}

/// Write fixed layout model record to the world storage.
///
/// # Arguments
/// * `model` - the model selector.
/// * `key` - the model record key.
/// * `values` - the model record values.
/// * `offset` - the start of model record values in the `values` parameter.
/// * `layout` - the model record layout.
#[inline(always)]
pub fn write_fixed_layout(
    model: felt252, key: felt252, values: Span<felt252>, ref offset: u32, layout: Span<u8>,
) {
    database::set(model, key, values, offset, layout);
    offset += layout.len();
}

/// Write array layout model record to the world storage.
///
/// # Arguments
/// * `model` - the model selector.
/// * `key` - the model record key.
/// * `values` - the model record values.
/// * `offset` - the start of model record values in the `values` parameter.
/// * `item_layout` - the model record layout (temporary a Span because of type recursion issue).
pub fn write_array_layout(
    model: felt252, key: felt252, values: Span<felt252>, ref offset: u32, item_layout: Span<Layout>,
) {
    assert((values.len() - offset) > 0, 'Invalid values length');

    // first, read array size which is the first felt252 from values
    let array_len = *values.at(offset);
    assert(array_len.into() <= database::MAX_ARRAY_LENGTH, 'invalid array length');

    // then, write the array size
    database::set_single(model, key, array_len);
    offset += 1;

    // and then, write array items
    let item_layout = *item_layout.at(0);
    let array_len: u32 = array_len.try_into().unwrap();

    for i in 0..array_len {
        let key = combine_key(key, i.into());
        write_layout(model, key, values, ref offset, item_layout);
    };
}


/// Write fixed array layout model record to the world storage.
///
/// # Arguments
/// * `model` - the model selector.
/// * `key` - the model record key.
/// * `values` - the model record values.
/// * `offset` - the start of model record values in the `values` parameter.
/// * `item_layout` - the model record layout (temporary a Span because of type recursion issue).
pub fn write_fixed_array_layout(
    model: felt252,
    key: felt252,
    values: Span<felt252>,
    ref offset: u32,
    mut item_layout: Span<(Layout, u32)>,
) {
    let (item_layout, array_len): (Layout, u32) = *item_layout.pop_front().unwrap();

    // Note: no need to write the array length as it is fixed at compile-time
    // and stored in the layout.
    for i in 0..array_len {
        write_layout(model, combine_key(key, i.into()), values, ref offset, item_layout);
    };
}

///
pub fn write_byte_array_layout(
    model: felt252, key: felt252, values: Span<felt252>, ref offset: u32,
) {
    // The ByteArray internal structure is
    // struct ByteArray {
    //    data: Array<bytes31>,
    //    pending_word: felt252,
    //    pending_word_len: usize,
    // }
    //
    // That means, the length of data to write from 'values' is:
    // 1 + len(data) + 1 + 1 = len(data) + 3
    assert((values.len() - offset) >= MIN_BYTE_ARRAY_SIZE, 'Invalid values length');

    let data_len = *values.at(offset);
    assert(
        data_len.into() <= (database::MAX_ARRAY_LENGTH - MIN_BYTE_ARRAY_SIZE.into()),
        'invalid array length',
    );

    let array_size: u32 = data_len.try_into().unwrap() + MIN_BYTE_ARRAY_SIZE.into();
    assert((values.len() - offset) >= array_size, 'Invalid values length');

    database::set_array(model, key, values, offset, array_size);
    offset += array_size;
}

/// Write struct layout model record to the world storage.
///
/// # Arguments
/// * `model` - the model selector.
/// * `key` - the model record key.
/// * `values` - the model record values.
/// * `offset` - the start of model record values in the `values` parameter.
/// * `layout` - list of field layouts.
pub fn write_struct_layout(
    model: felt252, key: felt252, values: Span<felt252>, ref offset: u32, layout: Span<FieldLayout>,
) {
    for field_layout in layout {
        let field_key = combine_key(key, *field_layout.selector);
        write_layout(model, field_key, values, ref offset, *field_layout.layout);
    }
}

/// Write tuple layout model record to the world storage.
///
/// # Arguments
/// * `model` - the model selector.
/// * `key` - the model record key.
/// * `values` - the model record values.
/// * `offset` - the start of model record values in the `values` parameter.
/// * `layout` - list of tuple item layouts.
pub fn write_tuple_layout(
    model: felt252, key: felt252, values: Span<felt252>, ref offset: u32, layout: Span<Layout>,
) {
    for (i, field_layout) in layout.into_iter().enumerate() {
        let key = combine_key(key, i.into());
        write_layout(model, key, values, ref offset, *field_layout);
    };
}

pub fn write_enum_layout(
    model: felt252,
    key: felt252,
    values: Span<felt252>,
    ref offset: u32,
    variant_layouts: Span<FieldLayout>,
) {
    assert!(values.len() - offset > 0, "offset is out of bounds for enum layout variant");

    let variant = *values.at(offset);

    // first, get the variant value from `values`
    assert(variant.into() < 256_u256, 'invalid variant value');

    // and write it
    database::set_single(model, key, variant);
    offset += 1;

    // find the corresponding layout and then write the full variant
    let variant_data_key = combine_key(key, variant);

    match find_field_layout(variant, variant_layouts) {
        Option::Some(layout) => write_layout(model, variant_data_key, values, ref offset, layout),
        Option::None => panic!("Unable to find the variant layout for variant {}", variant),
    };
}

/// Delete a fixed layout model record from the world storage.
///
/// # Arguments
///   * `model` - the model selector.
///   * `key` - the model record key.
///   * `layout` - the model layout
#[inline(always)]
pub fn delete_fixed_layout(model: felt252, key: felt252, layout: Span<u8>) {
    database::delete(model, key, layout);
}

/// Delete an array layout model record from the world storage.
///
/// # Arguments
///   * `model` - the model selector.
///   * `key` - the model record key.
#[inline(always)]
pub fn delete_array_layout(model: felt252, key: felt252) {
    // just set the array length to 0
    database::delete_single(model, key);
}

/// Delete a fixed array layout model record from the world storage.
///
/// # Arguments
///   * `model` - the model selector.
///   * `key` - the model record key.
///   * `layout` - the model layout.
pub fn delete_fixed_array_layout(model: felt252, key: felt252, mut layout: Span<(Layout, u32)>) {
    let (item_layout, array_len): (Layout, u32) = *layout.pop_front().unwrap();
    for i in 0..array_len {
        delete_layout(model, combine_key(key, i.into()), item_layout);
    }
}

///
pub fn delete_byte_array_layout(model: felt252, key: felt252) {
    // The ByteArray internal structure is
    // struct ByteArray {
    //    data: Array<bytes31>,
    //    pending_word: felt252,
    //    pending_word_len: usize,
    // }
    //

    // So, just set the 3 first values to 0 (len(data), pending_world and pending_word_len)
    database::set(
        model,
        key,
        [0, 0, 0].span(),
        0,
        [packing::PACKING_MAX_BITS, packing::PACKING_MAX_BITS, packing::PACKING_MAX_BITS].span(),
    );
}

/// Delete a model record from the world storage.
///
/// # Arguments
///   * `model` - the model selector.
///   * `key` - the model record key.
///   * `layout` - the model layout
pub fn delete_layout(model: felt252, key: felt252, layout: Layout) {
    match layout {
        Layout::Fixed(layout) => { delete_fixed_layout(model, key, layout); },
        Layout::Struct(layout) => { delete_struct_layout(model, key, layout); },
        Layout::Array(_) => { delete_array_layout(model, key); },
        Layout::FixedArray(layout) => { delete_fixed_array_layout(model, key, layout); },
        Layout::Tuple(layout) => { delete_tuple_layout(model, key, layout); },
        Layout::ByteArray => { delete_byte_array_layout(model, key); },
        Layout::Enum(layout) => { delete_enum_layout(model, key, layout); },
    }
}

/// Delete a struct layout model record from the world storage.
///
/// # Arguments
///   * `model` - the model selector.
///   * `key` - the model record key.
///   * `layout` - list of field layouts.
pub fn delete_struct_layout(model: felt252, key: felt252, layout: Span<FieldLayout>) {
    for field_layout in layout {
        let key = combine_key(key, *field_layout.selector);
        delete_layout(model, key, *field_layout.layout);
    }
}

/// Delete a tuple layout model record from the world storage.
///
/// # Arguments
///   * `model` - the model selector.
///   * `key` - the model record key.
///   * `layout` - list of tuple item layouts.
pub fn delete_tuple_layout(model: felt252, key: felt252, layout: Span<Layout>) {
    for (i, field_layout) in layout.into_iter().enumerate() {
        let key = combine_key(key, i.into());
        delete_layout(model, key, *field_layout);
    }
}

pub fn delete_enum_layout(model: felt252, key: felt252, variant_layouts: Span<FieldLayout>) {
    // read the variant value
    let variant = database::get_single(model, key);
    assert(variant.into() < 256_u256, 'invalid variant value');

    // reset the variant value
    database::delete_single(model, key);

    // find the corresponding layout and the delete the full variant
    let variant_data_key = combine_key(key, variant);

    match find_field_layout(variant, variant_layouts) {
        Option::Some(layout) => delete_layout(model, variant_data_key, layout),
        Option::None => {
            // In the legacy Dojo storage, variants start from 0, but with
            // the new Dojo storage (DojoStore trait), variants start from 1.
            // So, if `variant equals 0 and we cannot find the corresponding
            // field layout, we are in the new Dojo storage case and we can just continue
            // as the variant data are not set.
            if variant != 0 {
                panic!("Unable to find the variant layout for variant {}", variant);
            }
        },
    };
}

/// Read a model record.
///
/// # Arguments
///   * `model` - the model selector
///   * `key` - model record key.
///   * `read_data` - the read data.
///   * `layout` - the model layout
pub fn read_layout(model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Layout) {
    match layout {
        Layout::Fixed(layout) => read_fixed_layout(model, key, ref read_data, layout),
        Layout::Struct(layout) => read_struct_layout(model, key, ref read_data, layout),
        Layout::Array(layout) => read_array_layout(model, key, ref read_data, layout),
        Layout::FixedArray(layout) => read_fixed_array_layout(model, key, ref read_data, layout),
        Layout::Tuple(layout) => read_tuple_layout(model, key, ref read_data, layout),
        Layout::ByteArray => read_byte_array_layout(model, key, ref read_data),
        Layout::Enum(layout) => read_enum_layout(model, key, ref read_data, layout),
    };
}

/// Read a fixed layout model record.
///
/// # Arguments
///   * `model` - the model selector
///   * `key` - model record key.
///   * `read_data` - the read data.
///   * `layout` - the model layout
#[inline(always)]
pub fn read_fixed_layout(
    model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Span<u8>,
) {
    let mut data = database::get(model, key, layout);
    read_data.append_span(data);
}

/// Read an array layout model record.
///
/// # Arguments
///   * `model` - the model selector
///   * `key` - model record key.
///   * `read_data` - the read data.
///   * `layout` - the array item layout
pub fn read_array_layout(
    model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Span<Layout>,
) {
    // read number of array items
    let array_len = database::get_single(model, key);
    assert(array_len.into() <= database::MAX_ARRAY_LENGTH, 'invalid array length');

    read_data.append(array_len);

    let item_layout = *layout.at(0);
    let array_len: u32 = array_len.try_into().unwrap();

    for i in 0..array_len {
        let field_key = combine_key(key, i.into());
        read_layout(model, field_key, ref read_data, item_layout);
    };
}

/// Read a fixed array layout model record.
///
/// # Arguments
///   * `model` - the model selector
///   * `key` - model record key.
///   * `read_data` - the read data.
///   * `layout` - the array item layout
pub fn read_fixed_array_layout(
    model: felt252, key: felt252, ref read_data: Array<felt252>, mut layout: Span<(Layout, u32)>,
) {
    let (item_layout, array_len): (Layout, u32) = *layout.pop_front().unwrap();
    for i in 0..array_len {
        read_layout(model, combine_key(key, i.into()), ref read_data, item_layout);
    };
}

///
pub fn read_byte_array_layout(model: felt252, key: felt252, ref read_data: Array<felt252>) {
    // The ByteArray internal structure is
    // struct ByteArray {
    //    data: Array<bytes31>,
    //    pending_word: felt252,
    //    pending_word_len: usize,
    // }
    //
    // So, read the length of data and compute the full size to read

    let data_len = database::get_single(model, key);
    assert(
        data_len.into() <= (database::MAX_ARRAY_LENGTH - MIN_BYTE_ARRAY_SIZE.into()),
        'invalid array length',
    );

    let array_size: u32 = data_len.try_into().unwrap() + MIN_BYTE_ARRAY_SIZE;

    let mut data = database::get_array(model, key, array_size);
    read_data.append_span(data);
}

/// Read a struct layout model record.
///
/// # Arguments
///   * `model` - the model selector
///   * `key` - model record key.
///   * `read_data` - the read data.
///   * `layout` - the list of field layouts.
pub fn read_struct_layout(
    model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Span<FieldLayout>,
) {
    for field_layout in layout {
        let field_key = combine_key(key, *field_layout.selector);
        read_layout(model, field_key, ref read_data, *field_layout.layout);
    }
}

/// Read a tuple layout model record.
///
/// # Arguments
///   * `model` - the model selector
///   * `key` - model record key.
///   * `read_data` - the read data.
///   * `layout` - the tuple item layouts
pub fn read_tuple_layout(
    model: felt252, key: felt252, ref read_data: Array<felt252>, layout: Span<Layout>,
) {
    for (i, field_layout) in layout.into_iter().enumerate() {
        let field_key = combine_key(key, i.into());
        read_layout(model, field_key, ref read_data, *field_layout);
    };
}

pub fn read_enum_layout(
    model: felt252, key: felt252, ref read_data: Array<felt252>, variant_layouts: Span<FieldLayout>,
) {
    // read the variant value first
    let variant = database::get_single(model, key);
    assert(variant.into() < 256_u256, 'invalid variant value');

    read_data.append(variant);

    // find the corresponding layout and the read the variant data
    let variant_data_key = combine_key(key, variant);

    match find_field_layout(variant, variant_layouts) {
        Option::Some(layout) => read_layout(model, variant_data_key, ref read_data, layout),
        Option::None => {
            // In the legacy Dojo storage, variants start from 0, but with
            // the new Dojo storage (DojoStore trait), variants start from 1.
            // So, if `variant equals 0 and we cannot find the corresponding
            // field layout, we are in the new Dojo storage case and we have to return
            // 0 to indicate an uninitialized variant.
            if variant != 0 {
                panic!("Unable to find the variant layout for variant {}", variant)
            }
        },
    };
}
