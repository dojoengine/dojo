use blockifier::execution::contract_class::ContractClassV1;
use cairo_lang_starknet::casm_contract_class::CasmContractClass;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use katana_db::codecs::{Compress, Decompress};
use katana_db::models::class::StoredContractClass;
use katana_primitives::contract::CompiledContractClass;

fn compress_contract(contract: CompiledContractClass) -> Vec<u8> {
    let class = StoredContractClass::from(contract);
    class.compress()
}

fn decompress_contract(compressed: &[u8]) -> CompiledContractClass {
    let class = StoredContractClass::decompress(compressed).unwrap();
    CompiledContractClass::from(class)
}

fn compress_contract_with_main_codec(c: &mut Criterion) {
    let class = {
        let class =
            serde_json::from_slice(include_bytes!("./artifacts/dojo_world_240.json")).unwrap();
        let class = CasmContractClass::from_contract_class(class, true).unwrap();
        CompiledContractClass::V1(ContractClassV1::try_from(class).unwrap())
    };

    c.bench_function("compress world contract", |b| {
        b.iter_with_large_drop(|| compress_contract(black_box(class.clone())))
    });
}

fn decompress_contract_with_main_codec(c: &mut Criterion) {
    let class = {
        let class =
            serde_json::from_slice(include_bytes!("./artifacts/dojo_world_240.json")).unwrap();
        let class = CasmContractClass::from_contract_class(class, true).unwrap();
        CompiledContractClass::V1(ContractClassV1::try_from(class).unwrap())
    };

    let compressed = compress_contract(class);

    c.bench_function("decompress world contract", |b| {
        b.iter_with_large_drop(|| decompress_contract(black_box(&compressed)))
    });
}

criterion_group!(contract, compress_contract_with_main_codec, decompress_contract_with_main_codec);
criterion_main!(contract);
