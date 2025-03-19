use std::env;

use criterion::{criterion_group, criterion_main, Criterion};
use katana_node_bindings::Katana;
use pprof::criterion::{Output, PProfProfiler};

fn benchmark_binary_startup(c: &mut Criterion) {
    let program = env::var("KATANA_BENCH_BIN_PATH").unwrap_or_else(|_| "katana".to_string());

    // Use the node bindings to launch Katana
    // The instance will be automatically killed when dropped
    //
    // Increase timeout for benchmark environment

    c.bench_function("Katana.Startup", |b| {
        b.iter_with_large_drop(|| Katana::new().path(&program).spawn());
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None))).sample_size(10);
    targets = benchmark_binary_startup
}

criterion_main!(benches);
