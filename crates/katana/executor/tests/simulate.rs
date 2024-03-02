mod fixtures;

use fixtures::cfg;
use fixtures::genesis;
use fixtures::signed_invoke_executable_tx;
use fixtures::state_provider;
use katana_executor::ExecutionOutput;
use katana_executor::ExecutorFactory;
use katana_executor::SimulationFlag;
use katana_primitives::block::GasPrices;
use katana_primitives::env::BlockEnv;
use katana_primitives::env::CfgEnv;
use katana_primitives::genesis::allocation::GenesisAllocation;
use katana_primitives::genesis::Genesis;
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_primitives::FieldElement;
use katana_provider::traits::state::StateProvider;
use starknet::macros::felt;

#[rstest::fixture]
fn block_env() -> BlockEnv {
    let l1_gas_prices = GasPrices { eth: 1000, strk: 1000 };
    BlockEnv { l1_gas_prices, sequencer_address: felt!("0x1").into(), ..Default::default() }
}

#[rstest::fixture]
fn executable_tx_without_max_fee(genesis: &Genesis, cfg: CfgEnv) -> ExecutableTxWithHash {
    let (addr, alloc) = genesis.allocations.first_key_value().expect("should have account");

    let GenesisAllocation::Account(account) = alloc else {
        panic!("should be account");
    };

    signed_invoke_executable_tx(
        *addr,
        account.private_key().unwrap(),
        cfg.chain_id,
        FieldElement::ZERO,
        FieldElement::ZERO,
    )
}

#[rstest::rstest]
// TODO: uncomment after fixing the invalid validate entry point retdata issue
// #[cfg_attr(feature = "sir", case::sir(fixtures::sir::factory::default()))]
#[cfg_attr(feature = "blockifier", case::blockifier(fixtures::blockifier::factory::default()))]
fn test_simulate_tx<EF: ExecutorFactory>(
    #[case] factory: EF,
    block_env: BlockEnv,
    #[from(state_provider)] state: Box<dyn StateProvider>,
    #[from(executable_tx_without_max_fee)] transaction: ExecutableTxWithHash,
) {
    let mut executor = factory.with_state_and_block_env(state, block_env);

    let res = executor.simulate(transaction, SimulationFlag::default()).expect("must simulate");
    assert!(res.gas_used() != 0, "gas must be consumed");
    assert!(res.actual_fee() != 0, "actual fee must be computed");

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
