use katana_primitives::block::BlockHashOrNumber;
use katana_provider::providers::db::DbProvider;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::transaction::{ReceiptProvider, TransactionProvider};
use katana_provider::BlockchainProvider;
use rstest_reuse::{self, *};

mod fixtures;
mod utils;

use fixtures::{db_provider, fork_provider, in_memory_provider};
use utils::generate_dummy_blocks_and_receipts;

#[template]
#[rstest::rstest]
#[case::insert_1_block(1)]
#[case::insert_2_block(2)]
#[case::insert_5_block(5)]
#[case::insert_10_block(10)]
fn insert_block_cases(#[case] block_count: u64) {}

#[apply(insert_block_cases)]
fn insert_block_with_in_memory_provider(
    #[from(in_memory_provider)] provider: BlockchainProvider<InMemoryProvider>,
    #[case] block_count: u64,
) -> anyhow::Result<()> {
    insert_block_test_impl(provider, block_count)
}

#[apply(insert_block_cases)]
fn insert_block_with_fork_provider(
    #[from(fork_provider)] provider: BlockchainProvider<ForkedProvider>,
    #[case] block_count: u64,
) -> anyhow::Result<()> {
    insert_block_test_impl(provider, block_count)
}

#[apply(insert_block_cases)]
fn insert_block_with_db_provider(
    #[from(db_provider)] provider: BlockchainProvider<DbProvider>,
    #[case] block_count: u64,
) -> anyhow::Result<()> {
    insert_block_test_impl(provider, block_count)
}

fn insert_block_test_impl<Db>(provider: BlockchainProvider<Db>, count: u64) -> anyhow::Result<()>
where
    Db: BlockProvider + BlockWriter + ReceiptProvider,
{
    let blocks = generate_dummy_blocks_and_receipts(count);

    for (block, receipts) in &blocks {
        provider.insert_block_with_states_and_receipts(
            block.clone(),
            Default::default(),
            receipts.clone(),
        )?;
    }

    for (block, receipts) in blocks {
        let block_id = BlockHashOrNumber::Hash(block.block.header.hash);
        let expected_block = block.block.unseal();

        let actual_block = provider.block(block_id)?;
        let actual_block_txs = provider.transactions_by_block(block_id)?;
        let actual_block_tx_count = provider.transaction_count_by_block(block_id)?;

        let actual_receipts = provider.receipts_by_block(block_id)?;

        assert_eq!(actual_receipts.as_ref().map(|r| r.len()), Some(expected_block.body.len()));
        assert_eq!(actual_receipts, Some(receipts));

        assert_eq!(actual_block_tx_count, Some(expected_block.body.len() as u64));
        assert_eq!(actual_block_txs, Some(expected_block.body.clone()));
        assert_eq!(actual_block, Some(expected_block));
    }

    Ok(())
}
