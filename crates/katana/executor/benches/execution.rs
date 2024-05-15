use std::time::Duration;

use blockifier::state::cached_state::{CachedState, GlobalContractCache};
use criterion::measurement::WallTime;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkGroup, Criterion};
use katana_executor::{SimulationFlag, StateProviderDb};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_provider::{test_utils, traits::state::StateFactoryProvider};
use pprof::criterion::{Output, PProfProfiler};

use crate::utils::{envs, tx};

mod utils;

fn executor_transact(c: &mut Criterion) {
    let mut group = c.benchmark_group("Invoke.ERC20.transfer");
    group.warm_up_time(Duration::from_millis(200));

    let provider = test_utils::test_in_memory_provider();
    let flags = SimulationFlag::new().skip_validate().skip_fee_transfer();

    let tx = tx();
    let (block_env, cfg_env) = envs();

    blockifier(&mut group, &provider, flags, (block_env, cfg_env), tx);
}

fn blockifier<SF>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    provider: SF,
    execution_flags: SimulationFlag,
    block_envs: (BlockEnv, CfgEnv),
    tx: ExecutableTxWithHash,
) where
    SF: StateFactoryProvider,
{
    use katana_executor::implementation::blockifier::utils::block_context_from_envs;
    use katana_executor::implementation::blockifier::utils::transact;

    // convert to blockifier block context
    let block_context = block_context_from_envs(&block_envs.0, &block_envs.1);

    group.bench_function("Blockifier", |b| {
        // we need to set up the cached state for each iteration
        b.iter_batched(
            || {
                // setup state
                let state = provider.latest().unwrap();

                // setup blockifier cached state
                let contract_cache = GlobalContractCache::new(100);
                let state = CachedState::new(StateProviderDb::from(state), contract_cache);

                (state, &block_context, &execution_flags, tx.clone())
            },
            |(mut state, block_context, flags, tx)| {
                transact(&mut state, &block_context, &flags, tx)
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
