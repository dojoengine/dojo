use criterion::{black_box, criterion_group, criterion_main, Criterion};
use katana_db::codecs::{Compress, Decompress};
use katana_primitives::class::CompiledClass;
use katana_primitives::utils::class::parse_compiled_class;

fn compress_contract(contract: CompiledClass) -> Vec<u8> {
    Compress::compress(contract)
}

fn decompress_contract(compressed: &[u8]) -> CompiledClass {
    <CompiledClass as Decompress>::decompress(compressed).unwrap()
}

fn compress_contract_with_main_codec(c: &mut Criterion) {
    let json = serde_json::from_str(include_str!("./artifacts/dojo_world_240.json")).unwrap();
    let class = parse_compiled_class(json).unwrap();

    c.bench_function("compress world contract", |b| {
        b.iter_with_large_drop(|| compress_contract(black_box(class.clone())))
    });
}

fn decompress_contract_with_main_codec(c: &mut Criterion) {
    let json = serde_json::from_str(include_str!("./artifacts/dojo_world_240.json")).unwrap();
    let class = parse_compiled_class(json).unwrap();
    let compressed = compress_contract(class);

    c.bench_function("decompress world contract", |b| {
        b.iter_with_large_drop(|| decompress_contract(black_box(&compressed)))
    });
}

criterion_group!(contract, compress_contract_with_main_codec, decompress_contract_with_main_codec);
criterion_main!(contract);
