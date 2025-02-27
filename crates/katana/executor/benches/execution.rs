use std::time::Duration;

use blockifier::state::cached_state::CachedState;
use criterion::measurement::WallTime;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkGroup, Criterion};
use katana_executor::implementation::blockifier::cache::ClassCache;
use katana_executor::implementation::blockifier::state::StateProviderDb;
use katana_executor::ExecutionFlags;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_provider::test_utils;
use katana_provider::traits::state::StateFactoryProvider;
use pprof::criterion::{Output, PProfProfiler};

use crate::utils::{envs, tx};

mod utils;

fn executor_transact(c: &mut Criterion) {
    let mut group = c.benchmark_group("Invoke.ERC20.transfer");
    group.warm_up_time(Duration::from_millis(200));

    let provider = test_utils::test_provider();
    let flags = ExecutionFlags::new();

    let tx = tx();
    let envs = envs();

    blockifier(&mut group, &provider, &flags, &envs, tx);
}

fn blockifier(
    group: &mut BenchmarkGroup<'_, WallTime>,
    provider: impl StateFactoryProvider,
    execution_flags: &ExecutionFlags,
    block_envs: &(BlockEnv, CfgEnv),
    tx: ExecutableTxWithHash,
) {
    use katana_executor::implementation::blockifier::utils::{block_context_from_envs, transact};

    // convert to blockifier block context
    let block_context = block_context_from_envs(&block_envs.0, &block_envs.1);

    group.bench_function("Blockifier.Cold", |b| {
        // we need to set up the cached state for each iteration as it's not cloneable
        b.iter_batched(
            || {
                // setup state
                let state = provider.latest().expect("failed to get latest state");
                let class_cache = ClassCache::new().unwrap();
                let state = CachedState::new(StateProviderDb::new(state, class_cache));

                (state, &block_context, execution_flags, tx.clone())
            },
            |(mut state, block_context, flags, tx)| {
                transact(&mut state, block_context, flags, tx, None)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = executor_transact
}

criterion_main!(benches);
