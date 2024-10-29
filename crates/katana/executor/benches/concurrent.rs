//! This benchmark is used to measure how much concurrency we can get when accessing the main
//! execution state for executing indepdenent transactions in parallel. This is useful to measure
//! how much concurrency we can get when the pending state is being accessed by multiple independent
//! requests.

use std::sync::Arc;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkGroup, Criterion};
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_executor::{ExecutionFlags, ExecutorFactory};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_provider::test_utils;
use katana_provider::traits::state::StateFactoryProvider;
use pprof::criterion::{Output, PProfProfiler};
use rayon::ThreadPoolBuilder;

mod utils;
use utils::{envs, tx};

/// Right now, we guarantee that the transaction's execution will not fail/revert.
fn concurrent(c: &mut Criterion) {
    const CONCURRENCY_SIZE: usize = 1000;

    let mut group = c.benchmark_group("Concurrent.Simulate");
    group.warm_up_time(Duration::from_millis(200));

    let provider = test_utils::test_provider();
    let flags = ExecutionFlags::new().with_account_validation(false);

    let tx = tx();
    let envs = envs();

    blockifier(&mut group, CONCURRENCY_SIZE, &provider, flags.clone(), envs.clone(), tx);
}

fn blockifier(
    group: &mut BenchmarkGroup<'_, WallTime>,
    concurrency_size: usize,
    provider: impl StateFactoryProvider,
    flags: ExecutionFlags,
    (block_env, cfg_env): (BlockEnv, CfgEnv),
    tx: ExecutableTxWithHash,
) {
    let factory = Arc::new(BlockifierFactory::new(cfg_env, flags.clone()));

    group.bench_function("Blockifier.1", |b| {
        b.iter_batched(
            || {
                let state = provider.latest().expect("failed to get latest state");
                let executor = factory.with_state_and_block_env(state, block_env.clone());
                (executor, tx.clone(), flags.clone())
            },
            |(executor, tx, flags)| executor.simulate(vec![tx], flags),
            BatchSize::SmallInput,
        )
    });

    group.bench_function(format!("Blockifier.{concurrency_size}"), |b| {
        // Setup the inputs for each thread to remove the overhead of creating the execution context
        // for every thread inside the benchmark.
        b.iter_batched(
            || {
                let state = provider.latest().expect("failed to get latest state");
                let executor = Arc::new(factory.with_state_and_block_env(state, block_env.clone()));
                let pool = ThreadPoolBuilder::new().num_threads(concurrency_size).build().unwrap();

                // setup inputs for each thread
                let mut fxs = Vec::with_capacity(concurrency_size);
                let mut handles = Vec::with_capacity(concurrency_size);

                for _ in 0..concurrency_size {
                    let (sender, rx) = oneshot::channel();
                    handles.push(rx);

                    let tx = tx.clone();
                    let flags = flags.clone();
                    let executor = Arc::clone(&executor);

                    fxs.push(move || {
                        let _ = executor.simulate(vec![tx], flags);
                        sender.send(()).unwrap();
                    });
                }

                (pool, fxs, handles)
            },
            |(pool, fxs, handles)| {
                for fx in fxs {
                    pool.spawn(fx);
                }

                for handle in handles {
                    handle.recv().unwrap();
                }
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = concurrent
}

criterion_main!(benches);
