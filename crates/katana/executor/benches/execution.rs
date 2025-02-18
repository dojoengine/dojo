use std::sync::Arc;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_executor::{BlockLimits, ExecutionFlags, ExecutorFactory};
use katana_provider::test_utils;
use katana_provider::traits::block::BlockNumberProvider;
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::StateFactoryProvider;
use pprof::criterion::{Output, PProfProfiler};

use crate::utils::{envs, setup, tx};

mod utils;

fn move_transactions(c: &mut Criterion) {
    let mut group = c.benchmark_group("SpawnAndMove");
    group.warm_up_time(Duration::from_millis(200));
    group.sample_size(10);

    let (node, tx_generator) = setup();
    let provider = node.backend.blockchain.provider();

    let state = Arc::new(provider.latest().unwrap());
    let latest_num = provider.latest_number().unwrap();
    let block_env = provider.block_env_at(latest_num.into()).unwrap().unwrap();

    group.bench_function("Blockifier.Move.1", |b| {
        b.iter_batched(
            || {
                let state = state.clone();
                let env = block_env.clone();
                let mut tx_generator = tx_generator.clone();

                let executor = node.backend.executor_factory.with_state_and_block_env(state, env);
                let txs = vec![tx_generator.move_tx()];

                (executor, txs)
            },
            |(mut executor, txs)| executor.execute_transactions(txs).expect("execution failed"),
            BatchSize::SmallInput,
        )
    });

    group.bench_function("Blockifier.Move.100", |b| {
        b.iter_batched(
            || {
                let state = state.clone();
                let env = block_env.clone();
                let mut tx_generator = tx_generator.clone();

                let executor = node.backend.executor_factory.with_state_and_block_env(state, env);
                let txs = (0..100).map(|_| tx_generator.move_tx()).collect::<Vec<_>>();

                (executor, txs)
            },
            |(mut executor, txs)| executor.execute_transactions(txs).expect("execution failed"),
            BatchSize::SmallInput,
        )
    });
}

fn erc20_transfer(c: &mut Criterion) {
    let txs = vec![tx()];
    let (block_env, cfg_env) = envs();
    let provider = test_utils::test_provider();

    let factory = BlockifierFactory::new(cfg_env, ExecutionFlags::default(), BlockLimits::max());

    c.bench_function("Invoke.ERC20.transfer", |b| {
        // we need to set up the cached state for each iteration as it's not cloneable
        b.iter_batched(
            || {
                let state = provider.latest().expect("failed to get latest state");
                let executor = factory.with_state_and_block_env(state, block_env.clone());
                (executor, txs.clone())
            },
            |(mut executor, txs)| executor.execute_transactions(txs).expect("execution failed"),
            BatchSize::SmallInput,
        )
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_millis(200)).with_profiler(PProfProfiler::new(100, Output::Flamegraph(None)));
    targets = erc20_transfer, move_transactions
}

criterion_main!(benches);
