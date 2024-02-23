mod fixtures;

use std::collections::HashMap;

use fixtures::{state_provider, valid_blocks};
use katana_executor::{ExecutionOutput, ExecutorFactory};
use katana_primitives::block::ExecutableBlock;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::constant::{
    DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH, DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
};
use katana_primitives::transaction::TxWithHash;
use katana_provider::traits::state::StateProvider;
use starknet::core::utils::{get_udc_deployed_address, UdcUniqueSettings, UdcUniqueness};
use starknet::macros::felt;

fn test_executor_with_valid_blocks_impl<EF: ExecutorFactory>(
    factory: EF,
    state: Box<dyn StateProvider>,
    blocks: [ExecutableBlock; 3],
) {
    let mut executor = factory.with_state(state);
    let mut expected_txs: Vec<TxWithHash> = Vec::with_capacity(3);

    // execute the blocks and assert the block env is equivalent to the values in the header

    {
        let block = &blocks[0];
        expected_txs.extend(block.body.iter().map(|t| t.into()));

        executor.execute_block(block.clone()).unwrap();

        // assert that the block env is correctly set
        let actual_block_env = executor.block_env();
        assert_eq!(actual_block_env.number, block.header.number);
        assert_eq!(actual_block_env.timestamp, block.header.timestamp);
        assert_eq!(actual_block_env.l1_gas_prices, block.header.gas_prices);
        assert_eq!(actual_block_env.sequencer_address, block.header.sequencer_address);
    }

    {
        let block = &blocks[1];
        expected_txs.extend(block.body.iter().map(|t| t.into()));

        executor.execute_block(block.clone()).unwrap();

        // assert that the block env is correctly set
        let actual_block_env = executor.block_env();
        assert_eq!(actual_block_env.number, block.header.number);
        assert_eq!(actual_block_env.timestamp, block.header.timestamp);
        assert_eq!(actual_block_env.l1_gas_prices, block.header.gas_prices);
        assert_eq!(actual_block_env.sequencer_address, block.header.sequencer_address);
    }

    {
        let block = &blocks[2];
        expected_txs.extend(block.body.iter().map(|t| t.into()));

        executor.execute_block(block.clone()).unwrap();

        // assert that the block env is correctly set
        let actual_block_env = executor.block_env();
        assert_eq!(actual_block_env.number, block.header.number);
        assert_eq!(actual_block_env.timestamp, block.header.timestamp);
        assert_eq!(actual_block_env.l1_gas_prices, block.header.gas_prices);
        assert_eq!(actual_block_env.sequencer_address, block.header.sequencer_address);
    }

    // get the current state of the executor after the blocks are executed
    let state = executor.state();

    // the contract address of the account deployed using the `DeployAccount` tx
    let new_acc: ContractAddress =
        felt!("0x77880e2192169bc7107d213ebe643452e1e3e8f40bcc2ebba420b77b1522bd1").into();

    // assert that the deploy account tx executed correctly
    let actual_new_acc_class_hash = state.class_hash_of_contract(new_acc).unwrap();
    let actual_new_acc_nonce = state.nonce(new_acc).unwrap();

    assert_eq!(
        actual_new_acc_class_hash,
        Some(DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH),
        "account should be deployed"
    );
    assert_eq!(actual_new_acc_nonce, Some(1u64.into()), "account nonce is updated");

    // the contract address of the main account used to send most of the transactions
    let main_account: ContractAddress =
        felt!("0x6b86e40118f29ebe393a75469b4d926c7a44c2e2681b6d319520b7c1156d114").into();

    // compute the contract address that we deploy thru the UDC using Invoke tx
    let deployed_contr = get_udc_deployed_address(
        felt!("0x6ea2ff5aa6f633708e69f5c61d2ac5f860d2435b46ddbd016aa065bce25100a"),
        felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f"),
        &UdcUniqueness::Unique(UdcUniqueSettings {
            deployer_address: *main_account,
            udc_contract_address: felt!(
                "0x41a78e741e5af2fec34b695679bc6891742439f7afb8484ecd7766661ad02bf"
            ),
        }),
        &[
            felt!("0x4b415249"),
            felt!("0x4b415249"),
            felt!("0x12"),
            felt!("0x1b39"),
            felt!("0x0"),
            felt!("0x6b86e40118f29ebe393a75469b4d926c7a44c2e2681b6d319520b7c1156d114"),
        ],
    );

    let class_hash = state.class_hash_of_contract(deployed_contr.into()).unwrap();
    assert_eq!(class_hash, Some(DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH), "contract is deployed");

    // assert that the nonce of the main contract is updated, 4 txs were executed
    let nonce = state.nonce(main_account).unwrap();
    assert_eq!(nonce, Some(3u64.into()), "account nonce is updated");

    // assert that the sierra class is declared
    let hash = felt!("0x420");
    let compiled_hash = felt!("0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc");

    let (casm, sierra) = fixtures::contract_class();
    let actual_casm = state.class(hash).unwrap();
    let actual_sierra = state.sierra_class(hash).unwrap();
    let actual_compiled_hash = state.compiled_class_hash_of_class_hash(hash).unwrap();

    assert_eq!(actual_casm, Some(casm));
    assert_eq!(actual_sierra, Some(sierra));
    assert_eq!(actual_compiled_hash, Some(compiled_hash));

    // assert the state updates

    let ExecutionOutput { states, transactions } = executor.take_execution_output().unwrap();

    // asserts that the executed transactions are stored
    let actual_txs: Vec<TxWithHash> = transactions.iter().map(|(tx, _)| tx.clone()).collect();

    assert_eq!(actual_txs, expected_txs);
    assert!(transactions.iter().all(|(_, rct)| rct.is_some()), "all txs should have a receipt");

    let actual_nonce_updates = states.state_updates.nonce_updates;
    let expected_nonce_updates = HashMap::from([(main_account, felt!("3")), (new_acc, felt!("1"))]);

    similar_asserts::assert_eq!(actual_nonce_updates, expected_nonce_updates);

    let actual_declared_classes = states.state_updates.declared_classes;
    let expected_declared_classes = HashMap::from([(
        felt!("0x420"),
        felt!("0x016c6081eb34ad1e0c5513234ed0c025b3c7f305902d291bad534cd6474c85bc"),
    )]);

    similar_asserts::assert_eq!(actual_declared_classes, expected_declared_classes);

    let actual_contract_deployed = states.state_updates.contract_updates;
    let expected_contract_deployed = HashMap::from([
        (new_acc, DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH),
        (deployed_contr.into(), DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH),
    ]);

    similar_asserts::assert_eq!(actual_contract_deployed, expected_contract_deployed);
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

#[cfg(feature = "sir")]
mod sir {
    use fixtures::sir::factory;
    use katana_executor::implementation::sir::NativeExecutorFactory;

    use super::*;

    // TODO: find out why cant deploy contract using UDC, ignore this test until it's fixed (possible an upstream issue)
    #[ignore]
    #[rstest::rstest]
    fn test_executor_with_valid_blocks(
        factory: NativeExecutorFactory,
        #[from(state_provider)] state: Box<dyn StateProvider>,
        #[from(valid_blocks)] blocks: [ExecutableBlock; 3],
    ) {
        test_executor_with_valid_blocks_impl(factory, state, blocks)
    }
}
