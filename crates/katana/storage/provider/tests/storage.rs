mod fixtures;

use anyhow::Result;
use fixtures::{fork_provider_with_spawned_fork_network, in_memory_provider, provider_with_states};
use katana_primitives::block::{BlockHashOrNumber, BlockNumber};
use katana_primitives::contract::{ContractAddress, StorageKey, StorageValue};
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::state::{StateFactoryProvider, StateProvider};
use katana_provider::BlockchainProvider;
use rstest_reuse::{self, *};
use starknet::macros::felt;

fn assert_state_provider_storage(
    state_provider: Box<dyn StateProvider>,
    expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
) -> Result<()> {
    for (address, key, expected_value) in expected_storage_entry {
        let actual_value = state_provider.storage(address, key)?;
        assert_eq!(actual_value, expected_value);
    }
    Ok(())
}

mod latest {
    use katana_provider::providers::db::DbProvider;

    use super::*;
    use crate::fixtures::db_provider;

    fn assert_latest_storage_value<Db: StateFactoryProvider>(
        provider: BlockchainProvider<Db>,
        expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) -> Result<()> {
        let state_provider = provider.latest()?;
        assert_state_provider_storage(state_provider, expected_storage_entry)
    }

    #[template]
    #[rstest::rstest]
    #[case(
        vec![
            (ContractAddress::from(felt!("1")), felt!("1"), Some(felt!("111"))),
            (ContractAddress::from(felt!("1")), felt!("2"), Some(felt!("222"))),
            (ContractAddress::from(felt!("1")), felt!("3"), Some(felt!("77"))),
            (ContractAddress::from(felt!("2")), felt!("1"), Some(felt!("12"))),
            (ContractAddress::from(felt!("2")), felt!("2"), Some(felt!("13")))
        ]
    )]
    fn test_latest_storage_read<Db>(
        #[from(provider_with_states)] provider: BlockchainProvider<Db>,
        #[case] storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) {
    }

    #[apply(test_latest_storage_read)]
    fn read_storage_from_in_memory_provider(
        #[with(in_memory_provider())] provider: BlockchainProvider<InMemoryProvider>,
        #[case] expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) -> Result<()> {
        assert_latest_storage_value(provider, expected_storage_entry)
    }

    #[apply(test_latest_storage_read)]
    fn read_storage_from_fork_provider_with_spawned_fork_network(
        #[with(fork_provider_with_spawned_fork_network::default())] provider: BlockchainProvider<
            ForkedProvider,
        >,
        #[case] expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) -> Result<()> {
        assert_latest_storage_value(provider, expected_storage_entry)
    }

    #[apply(test_latest_storage_read)]
    fn read_storage_from_db_provider(
        #[with(db_provider())] provider: BlockchainProvider<DbProvider>,
        #[case] expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) -> Result<()> {
        assert_latest_storage_value(provider, expected_storage_entry)
    }
}

mod historical {
    use katana_provider::providers::db::DbProvider;

    use super::*;
    use crate::fixtures::db_provider;

    fn assert_historical_storage_value<Db: StateFactoryProvider>(
        provider: BlockchainProvider<Db>,
        block_num: BlockNumber,
        expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) -> Result<()> {
        let state_provider = provider
            .historical(BlockHashOrNumber::Num(block_num))?
            .expect(ERROR_CREATE_HISTORICAL_PROVIDER);
        assert_state_provider_storage(state_provider, expected_storage_entry)
    }

    const ERROR_CREATE_HISTORICAL_PROVIDER: &str = "Failed to create historical state provider.";

    #[template]
    #[rstest::rstest]
    #[case::storage_at_block_0(
        0,
        vec![
            (ContractAddress::from(felt!("1")), felt!("1"), None),
            (ContractAddress::from(felt!("1")), felt!("2"), None),
            (ContractAddress::from(felt!("2")), felt!("1"), None),
            (ContractAddress::from(felt!("2")), felt!("2"), None)
        ])
    ]
    #[case::storage_at_block_1(
        1,
        vec![
            (ContractAddress::from(felt!("1")), felt!("1"), Some(felt!("100"))),
            (ContractAddress::from(felt!("1")), felt!("2"), Some(felt!("101"))),
            (ContractAddress::from(felt!("2")), felt!("1"), Some(felt!("200"))),
            (ContractAddress::from(felt!("2")), felt!("2"), Some(felt!("201"))),
        ])
    ]
    #[case::storage_at_block_4(
        4,
        vec![
            (ContractAddress::from(felt!("1")), felt!("1"), Some(felt!("111"))),
            (ContractAddress::from(felt!("1")), felt!("2"), Some(felt!("222"))),
            (ContractAddress::from(felt!("2")), felt!("1"), Some(felt!("200"))),
            (ContractAddress::from(felt!("2")), felt!("2"), Some(felt!("201"))),
        ])
    ]
    #[case::storage_at_block_5(
        5,
        vec![
            (ContractAddress::from(felt!("1")), felt!("1"), Some(felt!("111"))),
            (ContractAddress::from(felt!("1")), felt!("2"), Some(felt!("222"))),
            (ContractAddress::from(felt!("1")), felt!("3"), Some(felt!("77"))),
            (ContractAddress::from(felt!("2")), felt!("1"), Some(felt!("12"))),
            (ContractAddress::from(felt!("2")), felt!("2"), Some(felt!("13"))),
        ])
    ]
    fn test_historical_storage_read(
        #[from(provider_with_states)] provider: BlockchainProvider<InMemoryProvider>,
        #[case] block_num: BlockNumber,
        #[case] expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) {
    }

    #[apply(test_historical_storage_read)]
    fn read_storage_from_in_memory_provider(
        #[with(in_memory_provider())] provider: BlockchainProvider<InMemoryProvider>,
        #[case] block_num: BlockNumber,
        #[case] expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) -> Result<()> {
        assert_historical_storage_value(provider, block_num, expected_storage_entry)
    }

    #[apply(test_historical_storage_read)]
    fn read_storage_from_fork_provider_with_spawned_fork_network(
        #[with(fork_provider_with_spawned_fork_network::default())] provider: BlockchainProvider<
            ForkedProvider,
        >,
        #[case] block_num: BlockNumber,
        #[case] expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) -> Result<()> {
        assert_historical_storage_value(provider, block_num, expected_storage_entry)
    }

    #[apply(test_historical_storage_read)]
    fn read_storage_from_db_provider(
        #[with(db_provider())] provider: BlockchainProvider<DbProvider>,
        #[case] block_num: BlockNumber,
        #[case] expected_storage_entry: Vec<(ContractAddress, StorageKey, Option<StorageValue>)>,
    ) -> Result<()> {
        assert_historical_storage_value(provider, block_num, expected_storage_entry)
    }
}
