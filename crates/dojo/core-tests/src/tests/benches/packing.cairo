use dojo::storage::packing::*;
use crate::utils::GasCounterTrait;

fn build_unpack_and_layout(size: u32) -> (Span<felt252>, Span<u8>) {
    let mut unpacked = array![];
    let mut layout = array![];

    for _ in 0..size {
        unpacked.append_span([1, 2, 3, 4].span());
        layout.append_span([16, 128, 128, 8].span());
    }

    (unpacked.span(), layout.span())
}

fn build_pack_and_layout(size: u32) -> (Span<felt252>, Span<u8>) {
    let mut packed = array![];
    let mut layout = array![];

    for _ in 0..size {
        packed.append_span([1, 2].span());
        layout.append_span([16, 128, 128, 8].span());
    }

    (packed.span(), layout.span())
}

fn build_layout(size: u32) -> Span<u8> {
    let mut layout = array![];

    for _ in 0..size {
        layout.append_span([16, 128, 128, 8].span());
    }

    layout.span()
}

fn pack_bench_case(size: u32, label: ByteArray) {
    let mut packed = array![];
    let (mut unpacked, mut layout) = build_unpack_and_layout(size);

    let mut gas = GasCounterTrait::start();
    pack(ref packed, ref unpacked, 0, ref layout);
    gas.end_csv(format!("pack::{label}"));
}

fn unpack_bench_case(size: u32, label: ByteArray) {
    let mut unpacked = array![];
    let (mut packed, mut layout) = build_pack_and_layout(size);

    let mut gas = GasCounterTrait::start();
    unpack(ref unpacked, ref packed, ref layout);
    gas.end_csv(format!("unpack::{label}"));
}

fn calculate_packed_size_bench_case(size: u32, label: ByteArray) {
    let mut layout = build_layout(size);

    let mut gas = GasCounterTrait::start();
    calculate_packed_size(ref layout);
    gas.end_csv(format!("calculate_packed_size::{label}"));
}

#[test]
fn bench_calculate_packed_size() {
    calculate_packed_size_bench_case(8, "small");
    calculate_packed_size_bench_case(32, "medium");
    calculate_packed_size_bench_case(128, "large");
}

#[test]
fn bench_pack() {
    pack_bench_case(8, "small");
    pack_bench_case(32, "medium");
    pack_bench_case(128, "large");
}

#[test]
fn bench_unpack() {
    unpack_bench_case(8, "small");
    unpack_bench_case(32, "medium");
    unpack_bench_case(128, "large");
}

#[test]
fn bench_shl() {
    let mut gas = GasCounterTrait::start();
    shl(10, 10);
    gas.end_csv(format!("shl(10, 10)"));

    let mut gas = GasCounterTrait::start();
    shl(1, 255);
    gas.end_csv(format!("shl(1, 255)"));
}

#[test]
fn bench_shr() {
    let mut gas = GasCounterTrait::start();
    shr(10, 10);
    gas.end_csv(format!("shr(10, 10)"));

    let mut gas = GasCounterTrait::start();
    shr(core::num::traits::Bounded::MAX, 255);
    gas.end_csv(format!("shr(U256_MAX, 255)"));
}

#[test]
fn bench_pow2_const() {
    let mut gas = GasCounterTrait::start();
    pow2_const(10);
    gas.end_csv(format!("pow2_const(10)"));

    let mut gas = GasCounterTrait::start();
    pow2_const(255);
    gas.end_csv(format!("pow2_const(255)"));
}
