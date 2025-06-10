use dojo::storage::storage::{*, get_packed_array, set_packed_array};
use crate::utils::GasCounterTrait;
use super::utils::{build_span, build_span_from_values};

const KEYS: [felt252; 3] = [1, 2, 3];

fn build_layout(size: u32) -> Span<u8> {
    build_span_from_values(size, [16, 128, 128, 8].span())
}

fn build_values(size: u32) -> Span<felt252> {
    build_span_from_values(size, [1, 2, 3, 4].span())
}

fn get_many_bench_case(size: u32, label: ByteArray) {
    let mut layout = build_layout(size);

    let mut gas = GasCounterTrait::start();
    let _ = get_many(0, KEYS.span(), layout);
    gas.end_csv(format!("get_many::{label}"));
}

fn set_many_bench_case(size: u32, label: ByteArray) {
    let mut layout = build_layout(size);
    let mut values = build_values(size);

    let mut gas = GasCounterTrait::start();
    let _ = set_many(0, KEYS.span(), values, 0, layout);
    gas.end_csv(format!("set_many::{label}"));
}

fn get_packed_array_bench_case(size: u32, label: ByteArray) {
    let mut gas = GasCounterTrait::start();
    let _ = get_packed_array(0, KEYS.span(), size);
    gas.end_csv(format!("get_packed_array::{label}"));
}

fn set_packed_array_bench_case(size: u32, label: ByteArray) {
    let mut items = build_span(size, 1);

    let mut gas = GasCounterTrait::start();
    let _ = set_packed_array(0, KEYS.span(), items, 0, size);
    gas.end_csv(format!("set_packed_array::{label}"));
}

#[test]
fn bench_get_many() {
    get_many_bench_case(8, "small");
    get_many_bench_case(32, "medium");
    get_many_bench_case(128, "large");
}

#[test]
fn bench_set_many() {
    set_many_bench_case(8, "small");
    set_many_bench_case(32, "medium");
    set_many_bench_case(128, "large");
}

#[test]
fn bench_get_packed_array() {
    get_packed_array_bench_case(8, "small");
    get_packed_array_bench_case(32, "medium");
    get_packed_array_bench_case(128, "large");
}

#[test]
fn bench_set_packed_array() {
    set_packed_array_bench_case(8, "small");
    set_packed_array_bench_case(32, "medium");
    set_packed_array_bench_case(128, "large");
}

