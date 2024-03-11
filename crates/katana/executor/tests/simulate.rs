mod fixtures;

use fixtures::transaction::executable_tx;
use fixtures::{executor_factory, state_provider};
use katana_executor::{ExecutionOutput, ExecutorFactory, SimulationFlag};
use katana_primitives::block::GasPrices;
use katana_primitives::env::BlockEnv;
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_provider::traits::state::StateProvider;
use rstest_reuse::{self, *};
use starknet::macros::felt;

#[rstest::fixture]
fn block_env() -> BlockEnv {
    let l1_gas_prices = GasPrices { eth: 1000, strk: 1000 };
    BlockEnv { l1_gas_prices, sequencer_address: felt!("0x1").into(), ..Default::default() }
}

#[template]
#[rstest::rstest]
#[case::tx(executable_tx::default(), SimulationFlag::new())]
#[case::tx_skip_validate(executable_tx::default(), SimulationFlag::new().skip_validate())]
#[case::tx_no_signature_skip_validate(executable_tx::partial_1(false), SimulationFlag::new().skip_validate())]
#[should_panic]
#[case::tx_no_signature(executable_tx::partial_1(false), SimulationFlag::new())]
fn simulate_tx<EF: ExecutorFactory>(
    executor_factory: EF,
    block_env: BlockEnv,
    state_provider: Box<dyn StateProvider>,
    #[case] tx: ExecutableTxWithHash,
    #[case] flags: SimulationFlag,
) {
}

#[allow(unused)]
fn test_simulate_tx_impl<EF: ExecutorFactory>(
    executor_factory: EF,
    block_env: BlockEnv,
    state_provider: Box<dyn StateProvider>,
    tx: ExecutableTxWithHash,
    flags: SimulationFlag,
) {
    let mut executor = executor_factory.with_state_and_block_env(state_provider, block_env);

    // TODO: assert that the tx execution didn't fail
    let _ = executor.simulate(tx, flags).expect("must simulate");

    // check that the underlying state is not modified
    let ExecutionOutput { states, transactions } =
        executor.take_execution_output().expect("must take output");

    assert!(transactions.is_empty(), "simulated tx should not be stored");

    assert!(states.state_updates.nonce_updates.is_empty(), "no state updates");
    assert!(states.state_updates.storage_updates.is_empty(), "no state updates");
    assert!(states.state_updates.contract_updates.is_empty(), "no state updates");
    assert!(states.state_updates.declared_classes.is_empty(), "no state updates");

    assert!(states.declared_sierra_classes.is_empty(), "no new classes should be declared");
    assert!(states.declared_compiled_classes.is_empty(), "no new classes should be declared");
}

#[cfg(feature = "blockifier")]
mod blockifier {
    use fixtures::blockifier::factory;
    use katana_executor::implementation::blockifier::BlockifierFactory;

    use super::*;

    #[apply(simulate_tx)]
    fn test_simulate_tx(
        #[with(factory::default())] executor_factory: BlockifierFactory,
        block_env: BlockEnv,
        state_provider: Box<dyn StateProvider>,
        #[case] tx: ExecutableTxWithHash,
        #[case] flags: SimulationFlag,
    ) {
        test_simulate_tx_impl(executor_factory, block_env, state_provider, tx, flags);
    }
}

#[cfg(feature = "sir")]
mod sir {
    use fixtures::sir::factory;
    use katana_executor::implementation::sir::NativeExecutorFactory;

    use super::*;

    #[apply(simulate_tx)]
    fn test_simulate_tx(
        #[with(factory::default())] executor_factory: NativeExecutorFactory,
        block_env: BlockEnv,
        state_provider: Box<dyn StateProvider>,
        #[case] tx: ExecutableTxWithHash,
        #[case] flags: SimulationFlag,
    ) {
        test_simulate_tx_impl(executor_factory, block_env, state_provider, tx, flags);
    }
}
