use dojo::storage::packing::*;

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

    pack(ref packed, ref unpacked, 0, ref layout);
}

fn unpack_bench_case(size: u32, label: ByteArray) {
    let mut unpacked = array![];
    let (mut packed, mut layout) = build_pack_and_layout(size);

    unpack(ref unpacked, ref packed, ref layout);
}

fn calculate_packed_size_bench_case(size: u32, label: ByteArray) {
    let mut layout = build_layout(size);

    calculate_packed_size(ref layout);
}

#[test]
fn bench_calculate_packed_size_small() {
    calculate_packed_size_bench_case(8, "small");
}

#[test]
fn bench_calculate_packed_size_medium() {
    calculate_packed_size_bench_case(32, "medium");
}

#[test]
fn bench_calculate_packed_size_big() {
    calculate_packed_size_bench_case(128, "large");
}

#[test]
fn bench_pack_small() {
    pack_bench_case(8, "small");
}

#[test]
fn bench_pack_medium() {
    pack_bench_case(32, "medium");
}

#[test]
fn bench_pack_big() {
    pack_bench_case(128, "large");
}

#[test]
fn bench_unpack_small() {
    unpack_bench_case(8, "small");
}

#[test]
fn bench_unpack_medium() {
    unpack_bench_case(32, "medium");
}

#[test]
fn bench_unpack_big() {
    unpack_bench_case(128, "large");
}

#[test]
fn bench_shl_10() {
    shl(10, 10);
}

#[test]
fn bench_shl_max() {
    shl(1, 255);
}

#[test]
fn bench_shr_10() {
    shr(10, 10);
}

#[test]
fn bench_shr_max() {
    shr(core::num::traits::Bounded::MAX, 255);
}

#[test]
fn bench_pow2_const_10() {
    pow2_const(10);
}

#[test]
fn bench_pow2_const_255() {
    pow2_const(255);
}
