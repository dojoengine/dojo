use std::{str::FromStr, time::Duration};

use criterion::{criterion_group, criterion_main, Criterion};
use katana_primitives::class::ContractClass;
use pprof::criterion::{Output, PProfProfiler};

fn class_compilation(c: &mut Criterion) {
    let json = include_str!("../../contracts/build/account.json");
    let class = ContractClass::from_str(json).unwrap();

    c.bench_function("Class.Compilation.Account", |b| {
        b.iter_with_large_drop(|| {
            let _ = class.clone().compile().expect("failed to compile");
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_millis(200)).with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = class_compilation
}

criterion_main!(benches);
