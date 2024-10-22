mod fixtures;

use fixtures::transaction::executable_tx;
use fixtures::{executor_factory, state_provider};
use katana_executor::{ExecutionOutput, ExecutorFactory, SimulationFlag};
use katana_primitives::block::GasPrices;
use katana_primitives::env::BlockEnv;
use katana_primitives::fee::PriceUnit;
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
    let transactions = vec![tx];
    let mut executor = executor_factory.with_state_and_block_env(state_provider, block_env);

    let results = executor.simulate(transactions.clone(), flags.clone());
    let fees = executor.estimate_fee(transactions, flags);

    assert!(results.iter().all(|res| res.result.is_success()), "all txs should be successful");
    assert!(fees.iter().all(|res| {
        match res {
            // makes sure that the fee is non-zero
            Ok(fee) => {
                fee.gas_price != 0
                    && fee.gas_consumed != 0
                    && fee.overall_fee != 0
                    && fee.unit == PriceUnit::Wei // TODO: add a tx that use STRK
            }
            Err(_) => false,
        }
    }),);

    // check that the underlying state is not modified
    let ExecutionOutput { states, transactions, stats } =
        executor.take_execution_output().expect("must take output");

    assert_eq!(stats.l1_gas_used, 0, "no gas usage should be recorded");
    assert_eq!(stats.cairo_steps_used, 0, "no steps usage should be recorded");
    assert!(transactions.is_empty(), "simulated tx should not be stored");

    assert!(states.state_updates.nonce_updates.is_empty(), "no state updates");
    assert!(states.state_updates.storage_updates.is_empty(), "no state updates");
    assert!(states.state_updates.deployed_contracts.is_empty(), "no state updates");
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
