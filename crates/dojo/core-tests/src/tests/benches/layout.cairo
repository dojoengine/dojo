use dojo::meta::layout::*;
use dojo::meta::{FieldLayout, Layout};
use dojo::storage::layout::{*, write_array_layout, write_byte_array_layout, write_layout};
use crate::utils::GasCounterTrait;

const SMALL_FIXED_LAYOUT: u32 = 8;
const MEDIUM_FIXED_LAYOUT: u32 = 16;
const BIG_FIXED_LAYOUT: u32 = 64;

/// Build a fixed layout and its associated values from its size.
fn build_fixed_layout(size: u32) -> (Span<u8>, Span<felt252>) {
    let mut layout = array![];
    let mut values = array![];

    for _ in 0..size {
        layout.append(8);
        values.append(42);
    }

    (layout.span(), values.span())
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

/// Build an enum layout and its associated values from the number of variants
/// and the variant data size.
fn build_enum_layout(nb_of_variants: u32, variant_size: u32) -> (Span<FieldLayout>, Span<felt252>) {
    let mut layout = array![];
    let (field_layout, field_values) = build_fixed_layout(variant_size);

    for i in 0..nb_of_variants {
        layout
            .append(FieldLayout { selector: (i + 1).into(), layout: Layout::Fixed(field_layout) });
    }

    let mut values = array![nb_of_variants.into()];
    values.append_span(field_values);

    (layout.span(), values.span())
}

/// write a fixed layout of a specific size.
fn write_fixed_layout_bench_case(size: u32, label: ByteArray) {
    let (layout, values) = build_fixed_layout(size);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_fixed_layout(1, 42, values, ref offset, layout);
    gas.end_csv(format!("write_fixed_layout::{label}"));
}

/// write a struct layout with a specific number of members.
fn write_struct_layout_bench_case(nb_of_members: u32, label: ByteArray) {
    let mut layout = array![];
    let mut values = array![];

    let (field_layout, field_values) = build_fixed_layout(SMALL_FIXED_LAYOUT);

    for i in 0..nb_of_members {
        layout
            .append(FieldLayout { selector: (i + 1).into(), layout: Layout::Fixed(field_layout) });
        values.append_span(field_values);
    }

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_struct_layout(1, 42, values.span(), ref offset, layout.span());
    gas.end_csv(format!("write_struct_layout::{label}"));
}

/// write an array layout of a specific size.
fn write_array_layout_bench_case(size: u32, label: ByteArray) {
    let mut values = array![size.into()];

    let (field_layout, field_values) = build_fixed_layout(SMALL_FIXED_LAYOUT);

    for _ in 0..size {
        values.append_span(field_values);
    }

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_array_layout(1, 42, values.span(), ref offset, [Layout::Fixed(field_layout)].span());
    gas.end_csv(format!("write_array_layout::{label}"));
}

/// write a tuple layout of a specific size.
fn write_tuple_layout_bench_case(size: u32, label: ByteArray) {
    let mut layout = array![];
    let mut values = array![];

    let (field_layout, field_values) = build_fixed_layout(SMALL_FIXED_LAYOUT);

    for _ in 0..size {
        layout.append(Layout::Fixed(field_layout));
        values.append_span(field_values);
    }

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_tuple_layout(1, 42, values.span(), ref offset, [Layout::Fixed(field_layout)].span());
    gas.end_csv(format!("write_tuple_layout::{label}"));
}

/// write a byte array layout of a specific length (number of 'hello' word)
fn write_byte_array_layout_bench_case(size: u32, label: ByteArray) {
    let values = build_byte_array_values(5);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_byte_array_layout(1, 42, values, ref offset);
    gas.end_csv(format!("write_byte_array_layout::{label}"));
}

/// write an enum layout of a specific size.
/// Note: as the write_enum_layout search the variant layout
/// among the variant layout list, this bench case write the last
/// variant index.
fn write_enum_layout_bench_case(size: u32, label: ByteArray) {
    let (layout, values) = build_enum_layout(size, SMALL_FIXED_LAYOUT);

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_enum_layout(1, 42, values, ref offset, layout);
    gas.end_csv(format!("write_enum_layout::{label}"));
}

fn write_complex_layout_bench_case(size: u32, label: ByteArray) {
    let mut values = array![];
    let mut layout = array![];

    let nb_of_members = size * 2;
    let array_size = size * 4;
    let nb_of_variants = size * 2;
    let fixed_layout_size = size * 4;

    let (fixed_layout, fixed_values) = build_fixed_layout(fixed_layout_size);
    let (enum_layout, enum_values) = build_enum_layout(nb_of_variants, fixed_layout_size);
    let field_layout = Layout::Array(
        [
            Layout::Tuple(
                [Layout::ByteArray, Layout::Fixed(fixed_layout), Layout::Enum(enum_layout)].span(),
            )
        ]
            .span(),
    );

    let mut item_values = array![];
    item_values.append_span(build_byte_array_values(size * 4));
    item_values.append_span(fixed_values);
    item_values.append_span(enum_values);

    let mut field_values = array![array_size.into()];
    for _ in 0..array_size {
        field_values.append_span(item_values.span());
    }

    for i in 0..nb_of_members {
        layout.append(FieldLayout { selector: (i + 1).into(), layout: field_layout });
        values.append_span(field_values.span());
    }

    let mut offset = 0_u32;
    let mut gas = GasCounterTrait::start();
    write_layout(1, 42, values.span(), ref offset, Layout::Struct(layout.span()));
    gas.end_csv(format!("write_layout::{label}"));
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
    write_struct_layout_bench_case(8, "medium");
    write_struct_layout_bench_case(16, "big");
}

#[test]
fn bench_write_array_layout() {
    write_array_layout_bench_case(4, "small");
    write_array_layout_bench_case(16, "medium");
    write_array_layout_bench_case(64, "big");
}

#[test]
fn bench_write_tuple_layout() {
    write_tuple_layout_bench_case(4, "small");
    write_tuple_layout_bench_case(16, "medium");
    write_tuple_layout_bench_case(64, "big");
}

#[test]
fn bench_write_byte_array_layout() {
    write_byte_array_layout_bench_case(4, "small");
    write_byte_array_layout_bench_case(16, "medium");
    write_byte_array_layout_bench_case(64, "big");
}

#[test]
fn bench_write_enum_layout() {
    write_enum_layout_bench_case(4, "small");
    write_enum_layout_bench_case(16, "medium");
    write_enum_layout_bench_case(64, "big");
}

#[test]
fn bench_write_complex_layout() {
    write_complex_layout_bench_case(1, "small");
    write_complex_layout_bench_case(2, "medium");
    write_complex_layout_bench_case(4, "big");
}
// set_entity
// set_entities
// delete_entity
// delete_entities
// entity
// entities
// emit_event
// emit_events

// register_namespace(ref self: T, namespace: ByteArray);
// register_event(ref self: T, namespace: ByteArray, class_hash: ClassHash);
// register_model(ref self: T, namespace: ByteArray, class_hash: ClassHash);
// register_contract(
// register_external_contract(
// register_library(
// init_contract(ref self: T, selector: felt252, init_calldata: Span<felt252>);
// upgrade_event(ref self: T, namespace: ByteArray, class_hash: ClassHash);
// upgrade_model(ref self: T, namespace: ByteArray, class_hash: ClassHash);
// upgrade_contract(ref self: T, namespace: ByteArray, class_hash: ClassHash) -> ClassHash;
// upgrade_external_contract(

// is_owner(self: @T, resource: felt252, address: ContractAddress) -> bool;
// grant_owner(ref self: T, resource: felt252, address: ContractAddress);
// revoke_owner(ref self: T, resource: felt252, address: ContractAddress);
// owners_count(self: @T, resource: felt252) -> u64;
// is_writer(self: @T, resource: felt252, contract: ContractAddress) -> bool;
// grant_writer(ref self: T, resource: felt252, contract: ContractAddress);
// revoke_writer(ref self: T, resource: felt252, contract: ContractAddress);

// resource(self: @T, selector: felt252) -> Resource;
// uuid(ref self: T) -> usize;
// metadata(self: @T, resource_selector: felt252) -> ResourceMetadata;
// set_metadata(ref self: T, metadata: ResourceMetadata);


