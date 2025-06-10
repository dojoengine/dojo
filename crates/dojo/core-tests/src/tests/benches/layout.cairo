use dojo::meta::layout::*;
use dojo::meta::{FieldLayout, Layout};
use dojo::storage::layout::{*, write_array_layout, write_byte_array_layout, write_enum_layout};
use crate::utils::GasCounterTrait;
use super::utils::{build_span, build_span_from_values};

const SMALL_FIXED_LAYOUT: u32 = 8;
const MEDIUM_FIXED_LAYOUT: u32 = 16;
const BIG_FIXED_LAYOUT: u32 = 64;

const MODEL_ID: felt252 = 1;
const KEY: felt252 = 42;

const DEFAULT_LAYOUT_ITEM_SIZE: u8 = 8;
const DEFAULT_VALUE: felt252 = 42;

fn build_layout(size: u32) -> Span<u8> {
    build_span(size, DEFAULT_LAYOUT_ITEM_SIZE)
}

fn build_values(size: u32) -> Span<felt252> {
    build_span(size, DEFAULT_VALUE)
}

/// Build a struct layout of `nb_of_members` fixed layout members.
/// Each member is composed of `size` felt252 values.
fn build_struct_layout(size: u32, nb_of_members: u32) -> Span<FieldLayout> {
    let mut layout = array![];
    let field_layout = build_layout(size);

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
    ba.serialize(ref values);

    values.span()
}

/// Build a enum layout of `nb_of_variants` fixed layout members.
/// Each variant is composed of `variant_size` felt252 values.
fn build_enum_layout(nb_of_variants: u32, variant_size: u32) -> Span<FieldLayout> {
    let mut layout = array![];
    let field_layout = build_layout(variant_size);

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

/// Build a complex struct layout using array, tuple and enum layouts.
fn build_complex_layout(size: u32) -> Span<FieldLayout> {
    let mut layout = array![];

    let nb_of_members = size;
    let nb_of_variants = size;
    let fixed_layout_size = size;

    let fixed_layout = build_layout(fixed_layout_size);
    let enum_layout = build_enum_layout(nb_of_variants, fixed_layout_size);
    let field_layout = Layout::Array(
        [
            Layout::Tuple(
                [Layout::ByteArray, Layout::Fixed(fixed_layout), Layout::Enum(enum_layout)].span(),
            )
        ]
            .span(),
    );

    for i in 0..nb_of_members {
        layout.append(FieldLayout { selector: (i + 1).into(), layout: field_layout });
    }

    layout.span()
}

/// Build the values of a serialized complex struct (to match with the layout
/// built by `build_complex_layout`).
fn build_complex_values(size: u32) -> Span<felt252> {
    let mut values = array![];

    let nb_of_members = size;
    let nb_of_variants = size;
    let fixed_layout_size = size;
    let array_size = size;

    let fixed_values = build_values(fixed_layout_size);
    let enum_values = build_enum_values(nb_of_variants, fixed_layout_size);

    let mut item_values = array![];
    item_values.append_span(build_byte_array_values(size * 4));
    item_values.append_span(fixed_values);
    item_values.append_span(enum_values);

    let mut field_values = array![array_size.into()];
    for _ in 0..array_size {
        field_values.append_span(item_values.span());
    }

    for _ in 0..nb_of_members {
        values.append_span(field_values.span());
    }

    values.span()
}

/// Bench casefor the read_fixed_layout function.
fn read_fixed_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_layout(size);

    let mut data = array![];
    let mut gas = GasCounterTrait::start();
    read_fixed_layout(MODEL_ID, KEY, ref data, layout);
    gas.end_csv(format!("read_fixed_layout::{label}"));

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), layout.len());
}

/// Bench case for the read_struct_layout function.
fn read_struct_layout_bench_case(nb_of_members: u32, label: ByteArray) {
    let mut layout = build_struct_layout(SMALL_FIXED_LAYOUT, nb_of_members);

    let mut data = array![];
    let mut gas = GasCounterTrait::start();
    read_struct_layout(MODEL_ID, KEY, ref data, layout);
    gas.end_csv(format!("read_struct_layout::{label}"));

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), nb_of_members * SMALL_FIXED_LAYOUT);
}

/// Bench case for the read_array_layout function.
fn read_array_layout_bench_case(size: u32, label: ByteArray) {
    // write the array first to have the array length correctly set in the storage.
    let item_values = build_values(SMALL_FIXED_LAYOUT);
    let item_layout = build_layout(SMALL_FIXED_LAYOUT);
    let mut offset = 0_u32;

    let item_values = build_span_from_values(size, item_values);

    let mut values = array![size.into()];
    values.append_span(item_values);

    write_array_layout(
        MODEL_ID, KEY, values.span(), ref offset, [Layout::Fixed(item_layout)].span(),
    );

    let mut data = array![];
    let mut gas = GasCounterTrait::start();
    read_array_layout(MODEL_ID, KEY, ref data, [Layout::Fixed(item_layout)].span());
    gas.end_csv(format!("read_array_layout::{label}"));

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), SMALL_FIXED_LAYOUT * size + 1);
    assert_eq!(*data[0], size.into());
}

/// Bench case for the read_tuple_layout function.
fn read_tuple_layout_bench_case(size: u32, label: ByteArray) {
    let field_layout = build_layout(SMALL_FIXED_LAYOUT);
    let mut layout = build_span(size, Layout::Fixed(field_layout));

    let mut data = array![];
    let mut gas = GasCounterTrait::start();
    read_tuple_layout(MODEL_ID, KEY, ref data, layout);
    gas.end_csv(format!("read_tuple_layout::{label}"));

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
    let mut gas = GasCounterTrait::start();
    read_byte_array_layout(MODEL_ID, KEY, ref data);
    gas.end_csv(format!("read_byte_array_layout::{label}"));

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
    let mut gas = GasCounterTrait::start();
    read_enum_layout(MODEL_ID, KEY, ref data, layout);
    gas.end_csv(format!("read_enum_layout::{label}"));

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), size + 1);
}

/// Bench case for the read_layout function.
fn read_complex_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_complex_layout(size);
    let values = build_complex_values(size);
    let mut offset = 0_u32;

    write_layout(MODEL_ID, KEY, values, ref offset, Layout::Struct(layout));

    let mut data = array![];
    let mut gas = GasCounterTrait::start();
    read_layout(MODEL_ID, KEY, ref data, Layout::Struct(layout));
    gas.end_csv(format!("read_layout::{label}"));

    // be sure that the read data have the correct size.
    assert_eq!(data.len(), values.len());
}

/// Bench case for the write_fixed_layout function.
fn write_fixed_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_layout(size);
    let values = build_values(size);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_fixed_layout(MODEL_ID, KEY, values, ref offset, layout);
    gas.end_csv(format!("write_fixed_layout::{label}"));
}

/// Bench case for the write_struct_layout function.
fn write_struct_layout_bench_case(nb_of_members: u32, label: ByteArray) {
    let mut layout = build_struct_layout(SMALL_FIXED_LAYOUT, nb_of_members);
    let mut values = build_struct_values(SMALL_FIXED_LAYOUT, nb_of_members);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_struct_layout(MODEL_ID, KEY, values, ref offset, layout);
    gas.end_csv(format!("write_struct_layout::{label}"));
}

/// Bench case for the write_array_layout function.
fn write_array_layout_bench_case(size: u32, label: ByteArray) {
    let item_values = build_values(SMALL_FIXED_LAYOUT);
    let item_layout = build_layout(SMALL_FIXED_LAYOUT);

    let item_values = build_span_from_values(size, item_values);

    let mut values = array![size.into()];
    values.append_span(item_values);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_array_layout(
        MODEL_ID, KEY, values.span(), ref offset, [Layout::Fixed(item_layout)].span(),
    );
    gas.end_csv(format!("write_array_layout::{label}"));
}

/// Bench case for the write_tuple_layout function.
fn write_tuple_layout_bench_case(size: u32, label: ByteArray) {
    let field_layout = build_layout(SMALL_FIXED_LAYOUT);
    let field_values = build_values(SMALL_FIXED_LAYOUT);

    let mut layout = build_span(size, Layout::Fixed(field_layout));
    let mut values = build_span_from_values(size, field_values);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_tuple_layout(MODEL_ID, KEY, values, ref offset, layout);
    gas.end_csv(format!("write_tuple_layout::{label}"));
}

/// Bench case for the write_byte_array_layout function.
fn write_byte_array_layout_bench_case(size: u32, label: ByteArray) {
    let values = build_byte_array_values(5);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_byte_array_layout(MODEL_ID, KEY, values, ref offset);
    gas.end_csv(format!("write_byte_array_layout::{label}"));
}

/// Bench case for the write_enum_layout function.
fn write_enum_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_enum_layout(size, size);
    let values = build_enum_values(size, size);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_enum_layout(MODEL_ID, KEY, values, ref offset, layout);
    gas.end_csv(format!("write_enum_layout::{label}"));
}

/// Bench case for the write_layout function.
fn write_complex_layout_bench_case(size: u32, label: ByteArray) {
    let values = build_complex_values(size);
    let layout = build_complex_layout(size);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_layout(MODEL_ID, KEY, values, ref offset, Layout::Struct(layout));
    gas.end_csv(format!("write_layout::{label}"));
}

/// Bench case for the delete_fixed_layout function.
fn delete_fixed_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_layout(size);

    let mut gas = GasCounterTrait::start();
    delete_fixed_layout(MODEL_ID, KEY, layout);
    gas.end_csv(format!("delete_fixed_layout::{label}"));
}

/// Bench case for the delete_struct_layout function.
fn delete_struct_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_struct_layout(SMALL_FIXED_LAYOUT, size);

    let mut gas = GasCounterTrait::start();
    delete_struct_layout(MODEL_ID, KEY, layout);
    gas.end_csv(format!("delete_struct_layout::{label}"));
}

/// Bench case for the delete_array_layout function.
fn delete_array_layout_bench_case(size: u32, label: ByteArray) {
    let mut gas = GasCounterTrait::start();
    delete_array_layout(MODEL_ID, KEY);
    gas.end_csv(format!("delete_array_layout::{label}"));
}

/// Bench case for the delete_tuple_layout function.
fn delete_tuple_layout_bench_case(size: u32, label: ByteArray) {
    let field_layout = build_layout(SMALL_FIXED_LAYOUT);
    let layout = build_span(size, Layout::Fixed(field_layout));

    let mut gas = GasCounterTrait::start();
    delete_tuple_layout(MODEL_ID, KEY, layout);
    gas.end_csv(format!("delete_tuple_layout::{label}"));
}

/// Bench case for the delete_byte_array_layout function.
fn delete_byte_array_layout_bench_case(size: u32, label: ByteArray) {
    let mut offset = 0_u32;
    let values = build_byte_array_values(size);
    write_byte_array_layout(MODEL_ID, KEY, values, ref offset);

    let mut gas = GasCounterTrait::start();
    delete_byte_array_layout(MODEL_ID, KEY);
    gas.end_csv(format!("delete_byte_array_layout::{label}"));
}

/// Bench case for the delete_enum_layout function.
fn delete_enum_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_enum_layout(size, size);
    let values = build_enum_values(size, size);
    let mut offset = 0_u32;

    write_enum_layout(MODEL_ID, KEY, values, ref offset, layout);

    let mut gas = GasCounterTrait::start();
    delete_enum_layout(MODEL_ID, KEY, layout);
    gas.end_csv(format!("delete_enum_layout::{label}"));
}

/// Bench case for the delete_layout function.
fn delete_complex_layout_bench_case(size: u32, label: ByteArray) {
    let layout = build_complex_layout(size);
    let values = build_complex_values(size);
    let mut offset = 0_u32;

    write_layout(MODEL_ID, KEY, values, ref offset, Layout::Struct(layout));

    let mut gas = GasCounterTrait::start();
    delete_layout(MODEL_ID, KEY, Layout::Struct(layout));
    gas.end_csv(format!("delete_layout::{label}"));
}

#[test]
fn bench_write_fixed_layout() {
    write_fixed_layout_bench_case(SMALL_FIXED_LAYOUT, "small");
    write_fixed_layout_bench_case(MEDIUM_FIXED_LAYOUT, "medium");
    write_fixed_layout_bench_case(BIG_FIXED_LAYOUT, "big");
}

#[test]
fn bench_write_struct_layout() {
    write_struct_layout_bench_case(2, "small");
    write_struct_layout_bench_case(16, "medium");
    write_struct_layout_bench_case(64, "big");
}

#[test]
fn bench_write_array_layout() {
    write_array_layout_bench_case(4, "small");
    write_array_layout_bench_case(32, "medium");
    write_array_layout_bench_case(255, "big");
}

#[test]
fn bench_write_tuple_layout() {
    write_tuple_layout_bench_case(4, "small");
    write_tuple_layout_bench_case(32, "medium");
    write_tuple_layout_bench_case(255, "big");
}

#[test]
fn bench_write_byte_array_layout() {
    write_byte_array_layout_bench_case(4, "small");
    write_byte_array_layout_bench_case(32, "medium");
    write_byte_array_layout_bench_case(255, "big");
}

#[test]
fn bench_write_enum_layout() {
    write_enum_layout_bench_case(4, "small");
    write_enum_layout_bench_case(32, "medium");
    write_enum_layout_bench_case(255, "big");
}

#[test]
fn bench_write_complex_layout() {
    write_complex_layout_bench_case(1, "small");
    write_complex_layout_bench_case(4, "medium");
    write_complex_layout_bench_case(16, "big");
}

#[test]
fn bench_read_fixed_layout() {
    read_fixed_layout_bench_case(SMALL_FIXED_LAYOUT, "small");
    read_fixed_layout_bench_case(MEDIUM_FIXED_LAYOUT, "medium");
    read_fixed_layout_bench_case(BIG_FIXED_LAYOUT, "big");
}

#[test]
fn bench_read_struct_layout() {
    read_struct_layout_bench_case(2, "small");
    read_struct_layout_bench_case(16, "medium");
    read_struct_layout_bench_case(64, "big");
}

#[test]
fn bench_read_array_layout() {
    read_array_layout_bench_case(4, "small");
    read_array_layout_bench_case(32, "medium");
    read_array_layout_bench_case(255, "big");
}

#[test]
fn bench_read_tuple_layout() {
    read_tuple_layout_bench_case(4, "small");
    read_tuple_layout_bench_case(32, "medium");
    read_tuple_layout_bench_case(255, "big");
}

#[test]
fn bench_read_byte_array_layout() {
    read_byte_array_layout_bench_case(4, "small");
    read_byte_array_layout_bench_case(32, "medium");
    read_byte_array_layout_bench_case(255, "big");
}

#[test]
fn bench_read_enum_layout() {
    read_enum_layout_bench_case(4, "small");
    read_enum_layout_bench_case(32, "medium");
    read_enum_layout_bench_case(255, "big");
}

#[test]
fn bench_read_complex_layout() {
    read_complex_layout_bench_case(1, "small");
    read_complex_layout_bench_case(4, "medium");
    read_complex_layout_bench_case(16, "big");
}

#[test]
fn bench_delete_fixed_layout() {
    delete_fixed_layout_bench_case(SMALL_FIXED_LAYOUT, "small");
    delete_fixed_layout_bench_case(MEDIUM_FIXED_LAYOUT, "medium");
    delete_fixed_layout_bench_case(BIG_FIXED_LAYOUT, "big");
}

#[test]
fn bench_delete_struct_layout() {
    delete_struct_layout_bench_case(4, "small");
    delete_struct_layout_bench_case(16, "medium");
    delete_struct_layout_bench_case(64, "big");
}

#[test]
fn bench_delete_array_layout() {
    delete_array_layout_bench_case(4, "small");
    delete_array_layout_bench_case(32, "medium");
    delete_array_layout_bench_case(255, "big");
}

#[test]
fn bench_delete_tuple_layout() {
    delete_tuple_layout_bench_case(4, "small");
    delete_tuple_layout_bench_case(32, "medium");
    delete_tuple_layout_bench_case(255, "big");
}

#[test]
fn bench_delete_byte_array_layout() {
    delete_byte_array_layout_bench_case(4, "small");
    delete_byte_array_layout_bench_case(32, "medium");
    delete_byte_array_layout_bench_case(255, "big");
}

#[test]
fn bench_delete_enum_layout() {
    delete_enum_layout_bench_case(4, "small");
    delete_enum_layout_bench_case(32, "medium");
    delete_enum_layout_bench_case(255, "big");
}

#[test]
fn bench_delete_complex_layout() {
    delete_complex_layout_bench_case(1, "small");
    delete_complex_layout_bench_case(4, "medium");
    delete_complex_layout_bench_case(16, "big");
}
