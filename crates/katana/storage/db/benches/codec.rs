use criterion::{black_box, criterion_group, criterion_main, Criterion};
use katana_db::codecs::{Compress, Decompress};
use katana_db::models::class::StoredContractClass;
use katana_primitives::contract::CompiledContractClass;
use katana_primitives::utils::class::parse_compiled_class;

fn compress_contract(contract: CompiledContractClass) -> Vec<u8> {
    StoredContractClass::from(contract).compress()
}

fn decompress_contract(compressed: &[u8]) -> CompiledContractClass {
    CompiledContractClass::from(StoredContractClass::decompress(compressed).unwrap())
}

fn compress_contract_with_main_codec(c: &mut Criterion) {
    let class = parse_compiled_class(include_str!("./artifacts/dojo_world_240.json")).unwrap();

    c.bench_function("compress world contract", |b| {
        b.iter_with_large_drop(|| compress_contract(black_box(class.clone())))
    });
}

fn decompress_contract_with_main_codec(c: &mut Criterion) {
    let class = parse_compiled_class(include_str!("./artifacts/dojo_world_240.json")).unwrap();
    let compressed = compress_contract(class);

    c.bench_function("decompress world contract", |b| {
        b.iter_with_large_drop(|| decompress_contract(black_box(&compressed)))
    });
}

criterion_group!(contract, compress_contract_with_main_codec, decompress_contract_with_main_codec);
criterion_main!(contract);
