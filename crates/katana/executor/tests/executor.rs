mod fixtures;

use std::collections::BTreeMap;

use fixtures::{state_provider, valid_blocks};
use katana_executor::{ExecutionOutput, ExecutionResult, ExecutorFactory};
use katana_primitives::block::ExecutableBlock;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::constant::{
    DEFAULT_ACCOUNT_CLASS_HASH, DEFAULT_FEE_TOKEN_ADDRESS, DEFAULT_LEGACY_ERC20_CLASS_HASH,
    DEFAULT_PREFUNDED_ACCOUNT_BALANCE, DEFAULT_UDC_ADDRESS,
};
use katana_primitives::transaction::TxWithHash;
use katana_primitives::{address, Felt};
use katana_provider::traits::state::StateProvider;
use starknet::core::utils::{
    get_storage_var_address, get_udc_deployed_address, UdcUniqueSettings, UdcUniqueness,
};
use starknet::macros::felt;

fn test_executor_with_valid_blocks_impl<EF: ExecutorFactory>(
    factory: EF,
    state: Box<dyn StateProvider>,
    blocks: [ExecutableBlock; 3],
) {
    let cfg_env = factory.cfg();

    // the contract address of the main account used to send most of the transactions (see the
    // `valid_blocks` fixture)
    let main_account =
        address!("0x6677fe62ee39c7b07401f754138502bab7fac99d2d3c5d37df7d1c6fab10819");
    // the contract address of the account deployed using the `DeployAccount` tx (see the
    // `valid_blocks` fixture)
    let new_acc = address!("0x3ddfa445a70b927497249f94ff7431fc2e2abc761a34417fd4891beb7c2db85");

    let mut executor = factory.with_state(state);
    let mut expected_txs: Vec<TxWithHash> = Vec::with_capacity(3);

    // block 1
    //

    let block = &blocks[0];
    expected_txs.extend(block.body.iter().map(|t| t.into()));

    executor.execute_block(block.clone()).unwrap();

    // assert that the block env is correctly set
    let actual_block_env = executor.block_env();
    assert_eq!(actual_block_env.number, block.header.number);
    assert_eq!(actual_block_env.timestamp, block.header.timestamp);
    assert_eq!(actual_block_env.l1_gas_prices, block.header.gas_prices);
    assert_eq!(actual_block_env.sequencer_address, block.header.sequencer_address);

    let transactions = executor.transactions();
    assert_eq!(transactions.len(), 2, "2 transactions were executed");

    // ensure that all transactions succeeded, if not panic with the error message and tx index
    let has_failed = transactions.iter().enumerate().find_map(|(i, (_, res))| {
        if let ExecutionResult::Failed { error } = res { Some((i, error)) } else { None }
    });

    if let Some((pos, error)) = has_failed {
        panic!("transaction at index {pos} failed: {error}");
    }

    // asserts that the states are updated correctly after executing the 1st block

    let state_provider = executor.state();

    // assert that the nonce of the main contract is updated, 3 txs were executed
    let nonce = state_provider.nonce(main_account).unwrap().expect("nonce should exist");
    assert_eq!(nonce, 2u64.into(), "account nonce is updated");

    let updated_main_acc_balance = state_provider
        .storage(
            cfg_env.fee_token_addresses.eth,
            // the storage slot of the lower half of the fee balance
            get_storage_var_address("ERC20_balances", &[main_account.into()]).unwrap(), // felt!("0x6e78596cd9cb5c7ef89ba020ffb848c0926c43c652ac5f9e219d0c8267caefe"),
        )
        .unwrap()
        .expect("storage should exist");

    let actual_new_acc_balance = state_provider
        .storage(
            cfg_env.fee_token_addresses.eth,
            // the storage slot of the lower half of the fee balance
            get_storage_var_address("ERC20_balances", &[new_acc.into()]).unwrap(),
        )
        .unwrap()
        .expect("storage should exist");

    assert!(
        updated_main_acc_balance < Felt::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE),
        "sender balance should decrease"
    );
    assert_eq!(actual_new_acc_balance, felt!("0x9999999999999999"), "account balance is updated");

    // assert that the sierra class is declared
    let expected_class_hash = felt!("0x420");

    let (casm, sierra) = fixtures::contract_class();
    let actual_casm = state_provider.class(expected_class_hash).unwrap();
    let actual_sierra = state_provider.sierra_class(expected_class_hash).unwrap();

    assert_eq!(actual_casm, Some(casm), "casm class should be declared");
    assert_eq!(actual_sierra, Some(sierra), "sierra class should be declared");

    let expected_compiled_class_hash =
        felt!("0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc");
    let actual_compiled_hash =
        state_provider.compiled_class_hash_of_class_hash(expected_class_hash).unwrap();
    assert_eq!(
        actual_compiled_hash,
        Some(expected_compiled_class_hash),
        "compiled hash should be declared"
    );

    // block 2
    //

    let block = &blocks[1];
    expected_txs.extend(block.body.iter().map(|t| t.into()));

    executor.execute_block(block.clone()).unwrap();

    // assert that the block env is correctly set
    let actual_block_env = executor.block_env();
    assert_eq!(actual_block_env.number, block.header.number);
    assert_eq!(actual_block_env.timestamp, block.header.timestamp);
    assert_eq!(actual_block_env.l1_gas_prices, block.header.gas_prices);
    assert_eq!(actual_block_env.sequencer_address, block.header.sequencer_address);

    let transactions = executor.transactions();
    assert_eq!(transactions.len(), 3, "3 transactions were executed");

    // asserts that the states are updated correctly after executing the 2nd block

    let state_provider = executor.state();

    // assert that the deploy account tx executed correctly
    let actual_new_acc_class_hash = state_provider.class_hash_of_contract(new_acc).unwrap();
    let actual_new_acc_nonce = state_provider.nonce(new_acc).unwrap();

    assert_eq!(
        actual_new_acc_class_hash,
        Some(DEFAULT_ACCOUNT_CLASS_HASH),
        "account should be deployed"
    );
    assert_eq!(actual_new_acc_nonce, Some(1u64.into()), "account nonce is updated");

    let updated_new_acc_balance = state_provider
        .storage(
            cfg_env.fee_token_addresses.eth,
            // the storage slot of the lower half of the fee balance
            felt!("0x7c8bacc8c8a7db5e5d4e22ab58750239183ae3e08b17a07a486f85fe8aee391"),
        )
        .unwrap()
        .expect("storage should exist");

    assert!(
        updated_new_acc_balance < felt!("0x9999999999999999"),
        "account balance should be updated"
    );

    // block 3
    //

    let block = &blocks[2];
    expected_txs.extend(block.body.iter().map(|t| t.into()));

    executor.execute_block(block.clone()).unwrap();

    // assert that the block env is correctly set
    let actual_block_env = executor.block_env();
    assert_eq!(actual_block_env.number, block.header.number);
    assert_eq!(actual_block_env.timestamp, block.header.timestamp);
    assert_eq!(actual_block_env.l1_gas_prices, block.header.gas_prices);
    assert_eq!(actual_block_env.sequencer_address, block.header.sequencer_address);

    let transactions = executor.transactions();
    assert_eq!(transactions.len(), 4, "+1 tx from block 3");

    // compute the contract address that we deploy thru the UDC using Invoke tx (the erc20 contract)
    let deployed_contract = get_udc_deployed_address(
        felt!("0x6ea2ff5aa6f633708e69f5c61d2ac5f860d2435b46ddbd016aa065bce25100a"),
        DEFAULT_LEGACY_ERC20_CLASS_HASH,
        &UdcUniqueness::Unique(UdcUniqueSettings {
            deployer_address: *main_account,
            udc_contract_address: DEFAULT_UDC_ADDRESS.into(),
        }),
        // constructor arguments (refer to the valid_blocks fixture for the contract deployment for
        // the meaning of these values)
        &[
            felt!("0x4b415249"),
            felt!("0x4b415249"),
            felt!("0x12"),
            felt!("0x1b39"),
            felt!("0x0"),
            felt!("0x6677fe62ee39c7b07401f754138502bab7fac99d2d3c5d37df7d1c6fab10819"),
        ],
    );

    let state_provider = executor.state();

    let actual_deployed_contract_class_hash =
        state_provider.class_hash_of_contract(deployed_contract.into()).unwrap();
    let actual_storage_value_1 = state_provider
        .storage(deployed_contract.into(), get_storage_var_address("ERC20_name", &[]).unwrap())
        .unwrap();
    let actual_storage_value_2 = state_provider
        .storage(deployed_contract.into(), get_storage_var_address("ERC20_symbol", &[]).unwrap())
        .unwrap();
    let actual_storage_value_3 = state_provider
        .storage(deployed_contract.into(), get_storage_var_address("ERC20_decimals", &[]).unwrap())
        .unwrap();
    let actual_storage_value_4 = state_provider
        .storage(
            deployed_contract.into(),
            get_storage_var_address("ERC20_total_supply", &[]).unwrap(),
        )
        .unwrap();
    let actual_storage_value_4_1 = state_provider
        .storage(
            deployed_contract.into(),
            get_storage_var_address("ERC20_total_supply", &[]).unwrap() + Felt::ONE,
        )
        .unwrap();
    let actual_storage_value_5 = state_provider
        .storage(
            deployed_contract.into(),
            get_storage_var_address("ERC20_balances", &[main_account.into()]).unwrap(),
        )
        .unwrap();

    assert_eq!(
        actual_deployed_contract_class_hash,
        Some(DEFAULT_LEGACY_ERC20_CLASS_HASH),
        "contract should be deployed"
    );
    assert_eq!(actual_storage_value_1, Some(felt!("0x4b415249")), "ERC_name should be set");
    assert_eq!(actual_storage_value_2, Some(felt!("0x4b415249")), "ERC_symbol should be set");
    assert_eq!(actual_storage_value_3, Some(felt!("0x12")), "ERC_decimals should be set");
    assert_eq!(
        actual_storage_value_4,
        Some(felt!("0x1b39")),
        "ERC_total_supply lower should be set"
    );
    assert_eq!(
        actual_storage_value_4_1,
        Some(felt!("0x0")),
        "ERC_total_supply higher should be set"
    );
    assert_eq!(
        actual_storage_value_5,
        Some(felt!("0x1b39")),
        "ERC_balances recepient should be set"
    );

    // assert the state updates after all the blocks are executed
    let mut actual_total_gas: u128 = 0;
    let mut actual_total_steps: u128 = 0;

    // assert the state updates
    let ExecutionOutput { states, transactions, stats } = executor.take_execution_output().unwrap();

    // asserts that the executed transactions are stored
    let actual_txs: Vec<TxWithHash> = transactions
        .iter()
        .map(|(tx, res)| {
            if let Some(fee) = res.receipt().map(|r| r.fee()) {
                actual_total_gas += fee.gas_consumed;
            }
            if let Some(rec) = res.receipt() {
                actual_total_steps += rec.resources_used().vm_resources.n_steps as u128;
            }
            tx.clone()
        })
        .collect();

    assert_eq!(actual_total_gas, stats.l1_gas_used);
    assert_eq!(actual_total_steps, stats.cairo_steps_used);
    assert_eq!(actual_txs, expected_txs);

    let actual_nonce_updates = states.state_updates.nonce_updates;
    let expected_nonce_updates =
        BTreeMap::from([(main_account, felt!("3")), (new_acc, felt!("1"))]);

    let actual_declared_classes = states.state_updates.declared_classes;
    let expected_declared_classes = BTreeMap::from([(
        felt!("0x420"),
        felt!("0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc"),
    )]);

    let actual_contract_deployed = states.state_updates.deployed_contracts;
    let expected_contract_deployed = BTreeMap::from([
        (new_acc, DEFAULT_ACCOUNT_CLASS_HASH),
        (deployed_contract.into(), DEFAULT_LEGACY_ERC20_CLASS_HASH),
    ]);

    similar_asserts::assert_eq!(actual_nonce_updates, expected_nonce_updates);
    similar_asserts::assert_eq!(actual_declared_classes, expected_declared_classes);
    similar_asserts::assert_eq!(actual_contract_deployed, expected_contract_deployed);

    // TODO: asserts the storage updates
    let actual_storage_updates = states.state_updates.storage_updates;
    assert_eq!(actual_storage_updates.len(), 3, "only 3 contracts whose storage should be updated");
    assert!(
        actual_storage_updates.contains_key(&DEFAULT_FEE_TOKEN_ADDRESS),
        "fee token storage must get updated"
    );
    assert!(
        actual_storage_updates.contains_key(&(deployed_contract.into())),
        "deployed contract storage must get updated"
    );
    assert!(
        actual_storage_updates.contains_key(&new_acc),
        "newly deployed account storage must get updated"
    );
}

#[cfg(feature = "blockifier")]
mod blockifier {
    use fixtures::blockifier::factory;
    use katana_executor::implementation::blockifier::BlockifierFactory;

    use super::*;

    #[rstest::rstest]
    fn test_executor_with_valid_blocks(
        factory: BlockifierFactory,
        #[from(state_provider)] state: Box<dyn StateProvider>,
        #[from(valid_blocks)] blocks: [ExecutableBlock; 3],
    ) {
        test_executor_with_valid_blocks_impl(factory, state, blocks)
    }
}
