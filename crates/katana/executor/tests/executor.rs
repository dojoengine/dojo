mod fixtures;

use fixtures::blockifier::factory as blockifier_factory;
use fixtures::{state_provider, valid_blocks};
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_executor::ExecutorFactory;
use katana_primitives::block::ExecutableBlock;
use katana_primitives::contract::ContractAddress;
use katana_primitives::genesis::constant::{
    DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH, DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH,
};
use katana_primitives::transaction::TxWithHash;
use katana_provider::traits::state::StateProvider;
use starknet::core::utils::{get_udc_deployed_address, UdcUniqueSettings, UdcUniqueness};
use starknet::macros::felt;

#[rstest::rstest]
fn test_blockifier_executor_with_valid_blocks(
    #[from(blockifier_factory)] factory: BlockifierFactory,
    #[from(state_provider)] state: Box<dyn StateProvider>,
    #[from(valid_blocks)] blocks: [ExecutableBlock; 3],
) {
    let mut executor = factory.with_state(state);
    let mut expected_txs: Vec<TxWithHash> = vec![];

    {
        let block_0 = &blocks[0];
        expected_txs.extend(block_0.body.iter().map(|t| t.into()));

        executor.execute_block(block_0.clone()).unwrap();

        // assert that the block env is correctly set
        let actual_block_env = executor.block_env();
        assert_eq!(actual_block_env.number, block_0.header.number);
        assert_eq!(actual_block_env.timestamp, block_0.header.timestamp);
        assert_eq!(actual_block_env.l1_gas_prices, block_0.header.gas_prices);
        assert_eq!(actual_block_env.sequencer_address, block_0.header.sequencer_address);
    }

    {
        let block_1 = &blocks[1];
        expected_txs.extend(block_1.body.iter().map(|t| t.into()));

        executor.execute_block(block_1.clone()).unwrap();

        // assert that the block env is correctly set
        let actual_block_env = executor.block_env();
        assert_eq!(actual_block_env.number, block_1.header.number);
        assert_eq!(actual_block_env.timestamp, block_1.header.timestamp);
        assert_eq!(actual_block_env.l1_gas_prices, block_1.header.gas_prices);
        assert_eq!(actual_block_env.sequencer_address, block_1.header.sequencer_address);
    }

    {
        let block_2 = &blocks[2];
        expected_txs.extend(block_2.body.iter().map(|t| t.into()));

        executor.execute_block(block_2.clone()).unwrap();

        // assert that the block env is correctly set
        let actual_block_env = executor.block_env();
        assert_eq!(actual_block_env.number, block_2.header.number);
        assert_eq!(actual_block_env.timestamp, block_2.header.timestamp);
        assert_eq!(actual_block_env.l1_gas_prices, block_2.header.gas_prices);
        assert_eq!(actual_block_env.sequencer_address, block_2.header.sequencer_address);
    }

    let txs = executor.transactions();
    let actual_txs: Vec<TxWithHash> = txs.iter().map(|(tx, _)| tx.clone()).collect();

    // assertst that the executed transactions are stored
    assert_eq!(actual_txs, expected_txs);
    assert!(txs.iter().all(|(_, rct)| rct.is_some()), "all txs should have a receipt");

    let state = executor.state();

    // assert that the deploy account tx executed correctly
    {
        let account = ContractAddress(felt!(
            "0x77880e2192169bc7107d213ebe643452e1e3e8f40bcc2ebba420b77b1522bd1"
        ));

        let class_hash = state.class_hash_of_contract(account).unwrap();
        let nonce = state.nonce(account).unwrap();

        assert_eq!(class_hash, Some(DEFAULT_OZ_ACCOUNT_CONTRACT_CLASS_HASH), "account is deployed");
        assert_eq!(nonce, Some(1u64.into()), "account nonce is updated");
    }

    // assert that the invoke deploy tx executed correctly
    {
        let account = ContractAddress(felt!(
            "0x6b86e40118f29ebe393a75469b4d926c7a44c2e2681b6d319520b7c1156d114"
        ));

        let contract_address = get_udc_deployed_address(
            felt!("0x6ea2ff5aa6f633708e69f5c61d2ac5f860d2435b46ddbd016aa065bce25100a"),
            felt!("0x02a8846878b6ad1f54f6ba46f5f40e11cee755c677f130b2c4b60566c9003f1f"),
            &UdcUniqueness::Unique(UdcUniqueSettings {
                deployer_address: *account,
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

        let class_hash = state.class_hash_of_contract(contract_address.into()).unwrap();
        let nonce = state.nonce(account).unwrap();

        assert_eq!(
            class_hash,
            Some(DEFAULT_LEGACY_ERC20_CONTRACT_CLASS_HASH),
            "contract is deployed"
        );
        assert_eq!(nonce, Some(2u64.into()), "account nonce is updated");
    }
}
