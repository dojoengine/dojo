mod fixtures;

use anyhow::Result;
use fixtures::{
    db_provider, fork_provider_with_spawned_fork_network, in_memory_provider, provider_with_states,
};
use katana_primitives::block::{BlockHashOrNumber, BlockNumber};
use katana_primitives::class::ClassHash;
use katana_primitives::contract::{ContractAddress, Nonce};
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};
use katana_provider::BlockchainProvider;
use rstest_reuse::{self, *};
use starknet::macros::felt;

fn assert_state_provider_contract_info(
    state_provider: Box<dyn StateProvider>,
    expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
) -> Result<()> {
    for (address, expected_class_hash, expected_nonce) in expected_contract_info {
        let actual_class_hash = state_provider.class_hash_of_contract(address)?;
        let actual_nonce = state_provider.nonce(address)?;

        assert_eq!(actual_class_hash, expected_class_hash);
        assert_eq!(actual_nonce, expected_nonce);
    }

    Ok(())
}

mod latest {
    use katana_provider::providers::db::DbProvider;

    use super::*;

    fn assert_latest_contract_info<Db: StateFactoryProvider>(
        provider: BlockchainProvider<Db>,
        expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) -> Result<()> {
        let state_provider = provider.latest()?;
        assert_state_provider_contract_info(state_provider, expected_contract_info)
    }

    #[template]
    #[rstest::rstest]
    #[case(
        vec![
            (ContractAddress::from(felt!("1")), Some(felt!("22")), Some(felt!("3"))),
            (ContractAddress::from(felt!("2")), Some(felt!("33")), Some(felt!("2"))),
        ]
    )]
    fn test_latest_contract_info_read<Db>(
        #[from(provider_with_states)] provider: BlockchainProvider<Db>,
        #[case] expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) {
    }

    #[apply(test_latest_contract_info_read)]
    fn read_storage_from_in_memory_provider(
        #[with(in_memory_provider())] provider: BlockchainProvider<InMemoryProvider>,
        #[case] expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) -> Result<()> {
        assert_latest_contract_info(provider, expected_contract_info)
    }

    #[apply(test_latest_contract_info_read)]
    fn read_storage_from_fork_provider(
        #[with(fork_provider_with_spawned_fork_network::default())] provider: BlockchainProvider<
            ForkedProvider,
        >,
        #[case] expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) -> Result<()> {
        assert_latest_contract_info(provider, expected_contract_info)
    }

    #[apply(test_latest_contract_info_read)]
    fn read_storage_from_db_provider(
        #[with(db_provider())] provider: BlockchainProvider<DbProvider>,
        #[case] expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) -> Result<()> {
        assert_latest_contract_info(provider, expected_contract_info)
    }
}

mod historical {
    use katana_provider::providers::db::DbProvider;

    use super::*;

    fn assert_historical_contract_info<Db: StateFactoryProvider>(
        provider: BlockchainProvider<Db>,
        block_num: BlockNumber,
        expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) -> Result<()> {
        let state_provider = provider
            .historical(BlockHashOrNumber::Num(block_num))?
            .expect(ERROR_CREATE_HISTORICAL_PROVIDER);
        assert_state_provider_contract_info(state_provider, expected_contract_info)
    }

    const ERROR_CREATE_HISTORICAL_PROVIDER: &str = "Failed to create historical state provider.";

    #[template]
    #[rstest::rstest]
    #[case::storage_at_block_0(
        0,
        vec![
            (ContractAddress::from(felt!("1")), None, None),
            (ContractAddress::from(felt!("2")), None, None)
        ])
    ]
    #[case::storage_at_block_1(
        1,
        vec![
            (ContractAddress::from(felt!("1")), Some(felt!("11")), Some(felt!("1"))),
            (ContractAddress::from(felt!("2")), Some(felt!("11")), Some(felt!("1"))),
        ])
    ]
    #[case::storage_at_block_4(
        4,
        vec![
            (ContractAddress::from(felt!("1")), Some(felt!("11")), Some(felt!("2"))),
            (ContractAddress::from(felt!("2")), Some(felt!("22")), Some(felt!("1"))),
        ])
    ]
    #[case::storage_at_block_5(
        5,
        vec![
            (ContractAddress::from(felt!("1")), Some(felt!("22")), Some(felt!("3"))),
            (ContractAddress::from(felt!("2")), Some(felt!("33")), Some(felt!("2"))),
        ])
    ]
    fn test_historical_storage_read(
        #[from(provider_with_states)] provider: BlockchainProvider<InMemoryProvider>,
        #[case] block_num: BlockNumber,
        #[case] expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) {
    }

    #[apply(test_historical_storage_read)]
    fn read_storage_from_in_memory_provider(
        #[with(in_memory_provider())] provider: BlockchainProvider<InMemoryProvider>,
        #[case] block_num: BlockNumber,
        #[case] expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) -> Result<()> {
        assert_historical_contract_info(provider, block_num, expected_contract_info)
    }

    #[apply(test_historical_storage_read)]
    fn read_storage_from_fork_provider(
        #[with(fork_provider_with_spawned_fork_network::default())] provider: BlockchainProvider<
            ForkedProvider,
        >,
        #[case] block_num: BlockNumber,
        #[case] expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) -> Result<()> {
        assert_historical_contract_info(provider, block_num, expected_contract_info)
    }

    #[apply(test_historical_storage_read)]
    fn read_storage_from_db_provider(
        #[with(db_provider())] provider: BlockchainProvider<DbProvider>,
        #[case] block_num: BlockNumber,
        #[case] expected_contract_info: Vec<(ContractAddress, Option<ClassHash>, Option<Nonce>)>,
    ) -> Result<()> {
        assert_historical_contract_info(provider, block_num, expected_contract_info)
    }
}
