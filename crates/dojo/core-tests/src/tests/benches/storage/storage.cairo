use dojo::storage::storage::{*, get_packed_array, set_packed_array};
use crate::tests::benches::bench_utils::{build_span, build_span_from_values};

const KEYS: [felt252; 3] = [1, 2, 3];

fn build_layout(size: u32) -> Span<u8> {
    build_span_from_values(size, [16, 128, 128, 8].span())
}

fn build_values(size: u32) -> Span<felt252> {
    build_span_from_values(size, [1, 2, 3, 4].span())
}

fn get_many_bench_case(size: u32, label: ByteArray) {
    let mut layout = build_layout(size);
    let _ = get_many(0, KEYS.span(), layout);
}

fn set_many_bench_case(size: u32, label: ByteArray) {
    let mut layout = build_layout(size);
    let mut values = build_values(size);
    let _ = set_many(0, KEYS.span(), values, 0, layout);
}

fn get_packed_array_bench_case(size: u32, label: ByteArray) {
    let _ = get_packed_array(0, KEYS.span(), size);
}

fn set_packed_array_bench_case(size: u32, label: ByteArray) {
    let mut items = build_span(size, 1);
    let _ = set_packed_array(0, KEYS.span(), items, 0, size);
}

#[test]
fn bench_get_many_small() {
    get_many_bench_case(8, "small");
}

#[test]
fn bench_get_many_medium() {
    get_many_bench_case(32, "medium");
}

#[test]
fn bench_get_many_big() {
    get_many_bench_case(128, "large");
}

#[test]
fn bench_set_many_small() {
    set_many_bench_case(8, "small");
}

#[test]
fn bench_set_many_medium() {
    set_many_bench_case(32, "medium");
}

#[test]
fn bench_set_many_big() {
    set_many_bench_case(128, "large");
}

#[test]
fn bench_get_packed_array_small() {
    get_packed_array_bench_case(8, "small");
}

#[test]
fn bench_get_packed_array_medium() {
    get_packed_array_bench_case(32, "medium");
}

#[test]
fn bench_get_packed_array_big() {
    get_packed_array_bench_case(128, "large");
}

#[test]
fn bench_set_packed_array_small() {
    set_packed_array_bench_case(8, "small");
}

#[test]
fn bench_set_packed_array_medium() {
    set_packed_array_bench_case(32, "medium");
}

#[test]
fn bench_set_packed_array_big() {
    set_packed_array_bench_case(128, "large");
}
