use anyhow::Result;
use katana_primitives::block::{
    Block, BlockHashOrNumber, BlockNumber, BlockWithTxHashes, FinalityStatus,
};
use katana_primitives::env::BlockEnv;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_provider::providers::db::DbProvider;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::{
    BlockHashProvider, BlockProvider, BlockStatusProvider, BlockWriter,
};
use katana_provider::traits::env::BlockEnvProvider;
use katana_provider::traits::state::StateRootProvider;
use katana_provider::traits::state_update::StateUpdateProvider;
use katana_provider::traits::transaction::{
    ReceiptProvider, TransactionProvider, TransactionStatusProvider,
};
use katana_provider::BlockchainProvider;
use rstest_reuse::{self, *};

mod fixtures;
mod utils;

use fixtures::{
    db_provider, fork_provider, fork_provider_with_spawned_fork_network, in_memory_provider,
    mock_state_updates, provider_with_states,
};
use utils::generate_dummy_blocks_and_receipts;

#[apply(insert_block_cases)]
fn insert_block_with_in_memory_provider(
    #[from(in_memory_provider)] provider: BlockchainProvider<InMemoryProvider>,
    #[case] block_count: u64,
) -> Result<()> {
    insert_block_test_impl(provider, block_count)
}

#[apply(insert_block_cases)]
fn insert_block_with_fork_provider(
    #[from(fork_provider)] provider: BlockchainProvider<ForkedProvider>,
    #[case] block_count: u64,
) -> Result<()> {
    insert_block_test_impl(provider, block_count)
}

#[apply(insert_block_cases)]
fn insert_block_with_db_provider(
    #[from(db_provider)] provider: BlockchainProvider<DbProvider>,
    #[case] block_count: u64,
) -> Result<()> {
    insert_block_test_impl(provider, block_count)
}

fn insert_block_test_impl<Db>(provider: BlockchainProvider<Db>, count: u64) -> Result<()>
where
    Db: BlockProvider
        + BlockWriter
        + ReceiptProvider
        + StateRootProvider
        + TransactionStatusProvider
        + BlockEnvProvider,
{
    let blocks = generate_dummy_blocks_and_receipts(count);

    for (block, receipts) in &blocks {
        provider.insert_block_with_states_and_receipts(
            block.clone(),
            Default::default(),
            receipts.clone(),
        )?;
    }

    let actual_blocks_in_range = provider.blocks_in_range(0..=count)?;

    assert_eq!(actual_blocks_in_range.len(), count as usize);
    assert_eq!(
        actual_blocks_in_range,
        blocks.clone().into_iter().map(|b| b.0.block.unseal()).collect::<Vec<Block>>()
    );

    for (block, receipts) in blocks {
        let block_id = BlockHashOrNumber::Hash(block.block.header.hash);

        let expected_block_num = block.block.header.header.number;
        let expected_block_hash = block.block.header.hash;
        let expected_block = block.block.unseal();

        let expected_block_env = BlockEnv {
            number: expected_block_num,
            timestamp: expected_block.header.timestamp,
            l1_gas_prices: expected_block.header.gas_prices,
            sequencer_address: expected_block.header.sequencer_address,
        };

        let actual_block_hash = provider.block_hash_by_num(expected_block_num)?;

        let actual_block = provider.block(block_id)?;
        let actual_block_txs = provider.transactions_by_block(block_id)?;
        let actual_status = provider.block_status(block_id)?;
        let actual_state_root = provider.state_root(block_id)?;

        let actual_block_tx_count = provider.transaction_count_by_block(block_id)?;
        let actual_receipts = provider.receipts_by_block(block_id)?;

        let expected_block_with_tx_hashes = BlockWithTxHashes {
            header: expected_block.header.clone(),
            body: expected_block.body.clone().into_iter().map(|t| t.hash).collect(),
        };

        let actual_block_with_tx_hashes = provider.block_with_tx_hashes(block_id)?;
        let actual_block_env = provider.block_env_at(block_id)?;

        assert_eq!(actual_status, Some(FinalityStatus::AcceptedOnL2));
        assert_eq!(actual_block_with_tx_hashes, Some(expected_block_with_tx_hashes));

        for (idx, tx) in expected_block.body.iter().enumerate() {
            let actual_receipt = provider.receipt_by_hash(tx.hash)?;
            let actual_tx = provider.transaction_by_hash(tx.hash)?;
            let actual_tx_status = provider.transaction_status(tx.hash)?;
            let actual_tx_block_num_hash = provider.transaction_block_num_and_hash(tx.hash)?;
            let actual_tx_by_block_idx =
                provider.transaction_by_block_and_idx(block_id, idx as u64)?;

            assert_eq!(actual_tx_block_num_hash, Some((expected_block_num, expected_block_hash)));
            assert_eq!(actual_tx_status, Some(FinalityStatus::AcceptedOnL2));
            assert_eq!(actual_receipt, Some(receipts[idx].clone()));
            assert_eq!(actual_tx_by_block_idx, Some(tx.clone()));
            assert_eq!(actual_tx, Some(tx.clone()));
        }

        assert_eq!(actual_block_env, Some(expected_block_env));

        assert_eq!(actual_receipts.as_ref().map(|r| r.len()), Some(expected_block.body.len()));
        assert_eq!(actual_receipts, Some(receipts));

        assert_eq!(actual_block_tx_count, Some(expected_block.body.len() as u64));
        assert_eq!(actual_state_root, Some(expected_block.header.state_root));
        assert_eq!(actual_block_txs, Some(expected_block.body.clone()));
        assert_eq!(actual_block_hash, Some(expected_block_hash));
        assert_eq!(actual_block, Some(expected_block));
    }

    Ok(())
}

#[apply(test_read_state_update)]
fn test_read_state_update_with_in_memory_provider(
    #[with(in_memory_provider())] provider: BlockchainProvider<InMemoryProvider>,
    #[case] block_num: BlockNumber,
    #[case] expected_state_update: StateUpdatesWithDeclaredClasses,
) -> Result<()> {
    test_read_state_update_impl(provider, block_num, expected_state_update)
}

#[apply(test_read_state_update)]
fn test_read_state_update_with_fork_provider(
    #[with(fork_provider_with_spawned_fork_network::default())] provider: BlockchainProvider<
        ForkedProvider,
    >,
    #[case] block_num: BlockNumber,
    #[case] expected_state_update: StateUpdatesWithDeclaredClasses,
) -> Result<()> {
    test_read_state_update_impl(provider, block_num, expected_state_update)
}

#[apply(test_read_state_update)]
fn test_read_state_update_with_db_provider(
    #[with(db_provider())] provider: BlockchainProvider<DbProvider>,
    #[case] block_num: BlockNumber,
    #[case] expected_state_update: StateUpdatesWithDeclaredClasses,
) -> Result<()> {
    test_read_state_update_impl(provider, block_num, expected_state_update)
}

fn test_read_state_update_impl<Db>(
    provider: BlockchainProvider<Db>,
    block_num: BlockNumber,
    expected_state_update: StateUpdatesWithDeclaredClasses,
) -> Result<()>
where
    Db: StateUpdateProvider,
{
    let actual_state_update = provider.state_update(BlockHashOrNumber::from(block_num))?;
    assert_eq!(actual_state_update, Some(expected_state_update.state_updates));
    Ok(())
}

#[template]
#[rstest::rstest]
#[case::insert_1_block(1)]
#[case::insert_2_block(2)]
#[case::insert_5_block(5)]
#[case::insert_10_block(10)]
fn insert_block_cases(#[case] block_count: u64) {}

#[template]
#[rstest::rstest]
#[case::state_update_at_block_1(1, mock_state_updates()[0].clone())]
#[case::state_update_at_block_2(2, mock_state_updates()[1].clone())]
#[case::state_update_at_block_3(3, StateUpdatesWithDeclaredClasses::default())]
#[case::state_update_at_block_5(5, mock_state_updates()[2].clone())]
fn test_read_state_update<Db>(
    #[from(provider_with_states)] provider: BlockchainProvider<Db>,
    #[case] block_num: BlockNumber,
    #[case] expected_state_update: StateUpdatesWithDeclaredClasses,
) {
}
