use dojo::meta::layout::*;
use dojo::meta::{FieldLayout, Layout};
use dojo::model::Model;
use dojo::storage::layout::{*, write_array_layout, write_byte_array_layout, write_enum_layout};
use crate::tests::benches::bench_data::*;
use crate::tests::benches::bench_utils::{build_span, build_span_from_values};

const SMALL_FIXED_LAYOUT: u32 = 8;
const MEDIUM_FIXED_LAYOUT: u32 = 16;
const BIG_FIXED_LAYOUT: u32 = 64;

const MODEL_ID: felt252 = 1;
const KEY: felt252 = 42;

const DEFAULT_LAYOUT_ITEM_SIZE: u8 = 8;
const DEFAULT_VALUE: felt252 = 42;

/// Build a fixed layout of `size` u8 values.
fn build_fixed_layout(size: u32) -> Span<u8> {
    build_span(size, DEFAULT_LAYOUT_ITEM_SIZE)
}

/// Build a span of `size` felt252 values.
fn build_values(size: u32) -> Span<felt252> {
    build_span(size, DEFAULT_VALUE)
}

/// Build a struct layout of `nb_of_members` fixed layout members.
/// Each member is composed of `size` felt252 values.
fn build_struct_layout(size: u32, nb_of_members: u32) -> Span<FieldLayout> {
    let mut layout = array![];
    let field_layout = build_fixed_layout(size);

    for i in 0..nb_of_members {
        layout
            .append(FieldLayout { selector: (i + 1).into(), layout: Layout::Fixed(field_layout) });
    }

    layout.span()
}

/// Build the values of a serialized struct (to match with the layout built by
/// `build_struct_layout`)
fn build_struct_values(size: u32, nb_of_members: u32) -> Span<felt252> {
    let mut values = array![];
    let field_values = build_values(size);

    for _ in 0..nb_of_members {
        values.append_span(field_values);
    }

    values.span()
}

/// Build a serialized ByteArray composed of a specified number of
/// 'hello' word.
fn build_byte_array_values(nb_of_words: u32) -> Span<felt252> {
    let mut ba = "";

    for _ in 0..nb_of_words {
        ba.append_word('hello', 5);
    }

    let mut values = array![];
    core::serde::Serde::serialize(@ba, ref values);

    values.span()
}

/// Build a enum layout of `nb_of_variants` fixed layout members.
/// Each variant is composed of `variant_size` felt252 values.
fn build_enum_layout(nb_of_variants: u32, variant_size: u32) -> Span<FieldLayout> {
    let mut layout = array![];
    let field_layout = build_fixed_layout(variant_size);

    for i in 0..nb_of_variants {
        layout
            .append(FieldLayout { selector: (i + 1).into(), layout: Layout::Fixed(field_layout) });
    }

    layout.span()
}

/// Build the values of a serialized enum (to match with the layout built by
/// `build_enum_layout`).
/// Note: as the enum layour functions search the variant layout
/// among the variant layout list, this bench case write the last
/// variant index.
fn build_enum_values(nb_of_variants: u32, variant_size: u32) -> Span<felt252> {
    let mut values = array![nb_of_variants.into()];
    values.append_span(build_values(variant_size));

    values.span()
}

/// Bench case for the read_fixed_layout function.
fn read_fixed_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_fixed_layout(size);

    let mut data = array![];
    read_fixed_layout(MODEL_ID, KEY, ref data, layout);

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), layout.len());
}

/// Bench case for the read_struct_layout function.
fn read_struct_layout_bench_case(nb_of_members: u32, label: ByteArray) {
    let mut layout = build_struct_layout(SMALL_FIXED_LAYOUT, nb_of_members);

    let mut data = array![];
    read_struct_layout(MODEL_ID, KEY, ref data, layout);

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), nb_of_members * SMALL_FIXED_LAYOUT);
}

/// Bench case for the read_array_layout function.
fn read_array_layout_bench_case(size: u32, label: ByteArray) {
    // write the array first to have the array length correctly set in the storage.
    let item_values = build_values(SMALL_FIXED_LAYOUT);
    let item_layout = build_fixed_layout(SMALL_FIXED_LAYOUT);
    let mut offset = 0_u32;

    let item_values = build_span_from_values(size, item_values);

    let mut values = array![size.into()];
    values.append_span(item_values);

    write_array_layout(
        MODEL_ID, KEY, values.span(), ref offset, [Layout::Fixed(item_layout)].span(),
    );

    let mut data = array![];
    read_array_layout(MODEL_ID, KEY, ref data, [Layout::Fixed(item_layout)].span());

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), SMALL_FIXED_LAYOUT * size + 1);
    assert_eq!(*data[0], size.into());
}

/// Bench case for the read_tuple_layout function.
fn read_tuple_layout_bench_case(size: u32, label: ByteArray) {
    let field_layout = build_fixed_layout(SMALL_FIXED_LAYOUT);
    let mut layout = build_span(size, Layout::Fixed(field_layout));

    let mut data = array![];
    read_tuple_layout(MODEL_ID, KEY, ref data, layout);

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), SMALL_FIXED_LAYOUT * size);
}

/// Bench case for the read_byte_array_layout function.
fn read_byte_array_layout_bench_case(size: u32, label: ByteArray) {
    // Need to write a byte array first
    let mut offset = 0_u32;
    let values = build_byte_array_values(size);
    write_byte_array_layout(MODEL_ID, KEY, values, ref offset);

    let mut data = array![];
    read_byte_array_layout(MODEL_ID, KEY, ref data);

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), values.len());
}

/// Bench case for the read_enum_layout function.
fn read_enum_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_enum_layout(size, size);
    let values = build_enum_values(size, size);
    let mut offset = 0_u32;

    write_enum_layout(MODEL_ID, KEY, values, ref offset, layout);

    let mut data = array![];
    read_enum_layout(MODEL_ID, KEY, ref data, layout);

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), size + 1);
}

/// Bench case for the read_layout function.
fn read_model_layout_bench_case<M, +Model<M>>(model: @M, label: ByteArray) {
    let values = model.serialized_values();
    let layout = Model::<M>::layout();

    let mut offset = 0_u32;

    write_layout(MODEL_ID, KEY, values, ref offset, layout);

    let mut data = array![];
    read_layout(MODEL_ID, KEY, ref data, layout);

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), values.len());
}

/// Bench case for the write_fixed_layout function.
fn write_fixed_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_fixed_layout(size);
    let values = build_values(size);

    let mut offset = 0_u32;
    write_fixed_layout(MODEL_ID, KEY, values, ref offset, layout);
}

/// Bench case for the write_struct_layout function.
fn write_struct_layout_bench_case(nb_of_members: u32, label: ByteArray) {
    let mut layout = build_struct_layout(SMALL_FIXED_LAYOUT, nb_of_members);
    let mut values = build_struct_values(SMALL_FIXED_LAYOUT, nb_of_members);

    let mut offset = 0_u32;
    write_struct_layout(MODEL_ID, KEY, values, ref offset, layout);
}

/// Bench case for the write_array_layout function.
fn write_array_layout_bench_case(size: u32, label: ByteArray) {
    let item_values = build_values(SMALL_FIXED_LAYOUT);
    let item_layout = build_fixed_layout(SMALL_FIXED_LAYOUT);

    let item_values = build_span_from_values(size, item_values);

    let mut values = array![size.into()];
    values.append_span(item_values);

    let mut offset = 0_u32;
    write_array_layout(
        MODEL_ID, KEY, values.span(), ref offset, [Layout::Fixed(item_layout)].span(),
    );
}

/// Bench case for the write_tuple_layout function.
fn write_tuple_layout_bench_case(size: u32, label: ByteArray) {
    let field_layout = build_fixed_layout(SMALL_FIXED_LAYOUT);
    let field_values = build_values(SMALL_FIXED_LAYOUT);

    let mut layout = build_span(size, Layout::Fixed(field_layout));
    let mut values = build_span_from_values(size, field_values);

    let mut offset = 0_u32;
    write_tuple_layout(MODEL_ID, KEY, values, ref offset, layout);
}

/// Bench case for the write_byte_array_layout function.
fn write_byte_array_layout_bench_case(size: u32, label: ByteArray) {
    let values = build_byte_array_values(5);

    let mut offset = 0_u32;
    write_byte_array_layout(MODEL_ID, KEY, values, ref offset);
}

/// Bench case for the write_enum_layout function.
fn write_enum_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_enum_layout(size, size);
    let values = build_enum_values(size, size);

    let mut offset = 0_u32;
    write_enum_layout(MODEL_ID, KEY, values, ref offset, layout);
}

/// Bench case for the write_layout function.
fn write_model_layout_bench_case<M, +Model<M>>(model: @M, label: ByteArray) {
    let values = Model::<M>::serialized_values(model);
    let layout = Model::<M>::layout();

    let mut offset = 0_u32;
    write_layout(MODEL_ID, KEY, values, ref offset, layout);
}

/// Bench case for the delete_fixed_layout function.
fn delete_fixed_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_fixed_layout(size);

    delete_fixed_layout(MODEL_ID, KEY, layout);
}

/// Bench case for the delete_struct_layout function.
fn delete_struct_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_struct_layout(SMALL_FIXED_LAYOUT, size);

    delete_struct_layout(MODEL_ID, KEY, layout);
}

/// Bench case for the delete_array_layout function.
fn delete_array_layout_bench_case(size: u32, label: ByteArray) {
    delete_array_layout(MODEL_ID, KEY);
}

/// Bench case for the delete_tuple_layout function.
fn delete_tuple_layout_bench_case(size: u32, label: ByteArray) {
    let field_layout = build_fixed_layout(SMALL_FIXED_LAYOUT);
    let layout = build_span(size, Layout::Fixed(field_layout));

    delete_tuple_layout(MODEL_ID, KEY, layout);
}

/// Bench case for the delete_byte_array_layout function.
fn delete_byte_array_layout_bench_case(size: u32, label: ByteArray) {
    let mut offset = 0_u32;
    let values = build_byte_array_values(size);
    write_byte_array_layout(MODEL_ID, KEY, values, ref offset);

    delete_byte_array_layout(MODEL_ID, KEY);
}

/// Bench case for the delete_enum_layout function.
fn delete_enum_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_enum_layout(size, size);
    let values = build_enum_values(size, size);
    let mut offset = 0_u32;

    write_enum_layout(MODEL_ID, KEY, values, ref offset, layout);

    delete_enum_layout(MODEL_ID, KEY, layout);
}

/// Bench case for the delete_layout function.
fn delete_model_layout_bench_case<M, +Model<M>>(model: @M, label: ByteArray) {
    let layout = Model::<M>::layout();
    let values = Model::<M>::serialized_values(model);
    let mut offset = 0_u32;

    write_layout(MODEL_ID, KEY, values, ref offset, layout);

    delete_layout(MODEL_ID, KEY, layout);
}

#[test]
fn bench_write_fixed_layout_small() {
    write_fixed_layout_bench_case(SMALL_FIXED_LAYOUT, "small");
}

#[test]
fn bench_write_fixed_layout_medium() {
    write_fixed_layout_bench_case(MEDIUM_FIXED_LAYOUT, "medium");
}

#[test]
fn bench_write_fixed_layout_big() {
    write_fixed_layout_bench_case(BIG_FIXED_LAYOUT, "big");
}

#[test]
fn bench_write_struct_layout_small() {
    write_struct_layout_bench_case(2, "small");
}

#[test]
fn bench_write_struct_layout_medium() {
    write_struct_layout_bench_case(16, "medium");
}

#[test]
fn bench_write_struct_layout_big() {
    write_struct_layout_bench_case(64, "big");
}

#[test]
fn bench_write_array_layout_small() {
    write_array_layout_bench_case(4, "small");
}

#[test]
fn bench_write_array_layout_medium() {
    write_array_layout_bench_case(32, "medium");
}

#[test]
fn bench_write_array_layout_big() {
    write_array_layout_bench_case(255, "big");
}

#[test]
fn bench_write_tuple_layout_small() {
    write_tuple_layout_bench_case(4, "small");
}

#[test]
fn bench_write_tuple_layout_medium() {
    write_tuple_layout_bench_case(32, "medium");
}

#[test]
fn bench_write_tuple_layout_big() {
    write_tuple_layout_bench_case(255, "big");
}

#[test]
fn bench_write_byte_array_layout_small() {
    write_byte_array_layout_bench_case(4, "small");
}

#[test]
fn bench_write_byte_array_layout_medium() {
    write_byte_array_layout_bench_case(32, "medium");
}

#[test]
fn bench_write_byte_array_layout_big() {
    write_byte_array_layout_bench_case(255, "big");
}

#[test]
fn bench_write_enum_layout_small() {
    write_enum_layout_bench_case(4, "small");
}

#[test]
fn bench_write_enum_layout_medium() {
    write_enum_layout_bench_case(32, "medium");
}

#[test]
fn bench_write_enum_layout_big() {
    write_enum_layout_bench_case(255, "big");
}

#[test]
fn bench_write_model_layout_small() {
    write_model_layout_bench_case(@build_small_model(), "small");
}

#[test]
fn bench_write_model_layout_medium() {
    write_model_layout_bench_case(@build_medium_model(), "medium");
}

#[test]
fn bench_write_model_layout_big() {
    write_model_layout_bench_case(@build_big_model(), "big");
}

#[test]
fn bench_read_fixed_layout_small() {
    read_fixed_layout_bench_case(SMALL_FIXED_LAYOUT, "small");
}

#[test]
fn bench_read_fixed_layout_medium() {
    read_fixed_layout_bench_case(MEDIUM_FIXED_LAYOUT, "medium");
}

#[test]
fn bench_read_fixed_layout_big() {
    read_fixed_layout_bench_case(BIG_FIXED_LAYOUT, "big");
}

#[test]
fn bench_read_struct_layout_small() {
    read_struct_layout_bench_case(2, "small");
}

#[test]
fn bench_read_struct_layout_medium() {
    read_struct_layout_bench_case(16, "medium");
}

#[test]
fn bench_read_struct_layout_big() {
    read_struct_layout_bench_case(64, "big");
}

#[test]
fn bench_read_array_layout_small() {
    read_array_layout_bench_case(4, "small");
}

#[test]
fn bench_read_array_layout_medium() {
    read_array_layout_bench_case(32, "medium");
}

#[test]
fn bench_read_array_layout_big() {
    read_array_layout_bench_case(255, "big");
}

#[test]
fn bench_read_tuple_layout_small() {
    read_tuple_layout_bench_case(4, "small");
}

#[test]
fn bench_read_tuple_layout_medium() {
    read_tuple_layout_bench_case(32, "medium");
}

#[test]
fn bench_read_tuple_layout_big() {
    read_tuple_layout_bench_case(255, "big");
}

#[test]
fn bench_read_byte_array_layout_small() {
    read_byte_array_layout_bench_case(4, "small");
}

#[test]
fn bench_read_byte_array_layout_medium() {
    read_byte_array_layout_bench_case(32, "medium");
}

#[test]
fn bench_read_byte_array_layout_big() {
    read_byte_array_layout_bench_case(255, "big");
}

#[test]
fn bench_read_enum_layout_small() {
    read_enum_layout_bench_case(4, "small");
}

#[test]
fn bench_read_enum_layout_medium() {
    read_enum_layout_bench_case(32, "medium");
}

#[test]
fn bench_read_enum_layout_big() {
    read_enum_layout_bench_case(255, "big");
}

#[test]
fn bench_read_model_layout_small() {
    read_model_layout_bench_case(@build_small_model(), "small");
}

#[test]
fn bench_read_model_layout_medium() {
    read_model_layout_bench_case(@build_medium_model(), "medium");
}

#[test]
fn bench_read_model_layout_big() {
    read_model_layout_bench_case(@build_big_model(), "big");
}

#[test]
fn bench_delete_fixed_layout_small() {
    delete_fixed_layout_bench_case(SMALL_FIXED_LAYOUT, "small");
}

#[test]
fn bench_delete_fixed_layout_medium() {
    delete_fixed_layout_bench_case(MEDIUM_FIXED_LAYOUT, "medium");
}

#[test]
fn bench_delete_fixed_layout_big() {
    delete_fixed_layout_bench_case(BIG_FIXED_LAYOUT, "big");
}

#[test]
fn bench_delete_struct_layout_small() {
    delete_struct_layout_bench_case(4, "small");
}

#[test]
fn bench_delete_struct_layout_medium() {
    delete_struct_layout_bench_case(16, "medium");
}

#[test]
fn bench_delete_struct_layout_big() {
    delete_struct_layout_bench_case(64, "big");
}

#[test]
fn bench_delete_array_layout_small() {
    delete_array_layout_bench_case(4, "small");
}

#[test]
fn bench_delete_array_layout_medium() {
    delete_array_layout_bench_case(32, "medium");
}

#[test]
fn bench_delete_array_layout_big() {
    delete_array_layout_bench_case(255, "big");
}

#[test]
fn bench_delete_tuple_layout_small() {
    delete_tuple_layout_bench_case(4, "small");
}

#[test]
fn bench_delete_tuple_layout_medium() {
    delete_tuple_layout_bench_case(32, "medium");
}

#[test]
fn bench_delete_tuple_layout_big() {
    delete_tuple_layout_bench_case(255, "big");
}

#[test]
fn bench_delete_byte_array_layout_small() {
    delete_byte_array_layout_bench_case(4, "small");
}

#[test]
fn bench_delete_byte_array_layout_medium() {
    delete_byte_array_layout_bench_case(32, "medium");
}

#[test]
fn bench_delete_byte_array_layout_big() {
    delete_byte_array_layout_bench_case(255, "big");
}

#[test]
fn bench_delete_enum_layout_small() {
    delete_enum_layout_bench_case(4, "small");
}

#[test]
fn bench_delete_enum_layout_medium() {
    delete_enum_layout_bench_case(32, "medium");
}

#[test]
fn bench_delete_enum_layout_big() {
    delete_enum_layout_bench_case(255, "big");
}

#[test]
fn bench_delete_model_layout_small() {
    delete_model_layout_bench_case(@build_small_model(), "small");
}

#[test]
fn bench_delete_model_layout_medium() {
    delete_model_layout_bench_case(@build_medium_model(), "medium");
}

#[test]
fn bench_delete_model_layout_big() {
    delete_model_layout_bench_case(@build_big_model(), "big");
}
