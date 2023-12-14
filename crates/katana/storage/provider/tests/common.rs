use katana_primitives::block::{
    Block, BlockHash, BlockHashOrNumber, FinalityStatus, Header, SealedBlockWithStatus,
};
use katana_primitives::transaction::{Tx, TxHash, TxWithHash};
use katana_primitives::FieldElement;
use katana_provider::providers::fork::ForkedProvider;
use katana_provider::providers::in_memory::InMemoryProvider;
use katana_provider::traits::block::{BlockProvider, BlockWriter};
use katana_provider::traits::transaction::TransactionProvider;
use katana_provider::BlockchainProvider;
use rstest_reuse::{self, *};

mod fixtures;

use fixtures::{fork_provider, in_memory_provider};

fn generate_dummy_txs(count: u64) -> Vec<TxWithHash> {
    let mut txs = Vec::with_capacity(count as usize);
    for _ in 0..count {
        txs.push(TxWithHash {
            hash: TxHash::from(rand::random::<u128>()),
            transaction: Tx::Invoke(Default::default()),
        });
    }
    txs
}

fn generate_dummy_blocks(count: u64) -> Vec<SealedBlockWithStatus> {
    let mut blocks = Vec::with_capacity(count as usize);
    let mut parent_hash: BlockHash = 0u8.into();

    for i in 0..count {
        let body = generate_dummy_txs(rand::random::<u64>() % 10);
        let header = Header { parent_hash, number: i, ..Default::default() };
        let block =
            Block { header, body }.seal_with_hash(FieldElement::from(rand::random::<u128>()));
        parent_hash = block.header.hash;

        blocks.push(SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 });
    }

    blocks
}

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
    insert_block_impl(provider, block_count)
}

#[apply(insert_block_cases)]
fn insert_block_with_fork_provider(
    #[from(fork_provider)] provider: BlockchainProvider<ForkedProvider>,
    #[case] block_count: u64,
) -> anyhow::Result<()> {
    insert_block_impl(provider, block_count)
}

fn insert_block_impl<Db>(provider: BlockchainProvider<Db>, count: u64) -> anyhow::Result<()>
where
    Db: BlockProvider + BlockWriter,
{
    let blocks = generate_dummy_blocks(count);

    for block in &blocks {
        provider.insert_block_with_states_and_receipts(
            block.clone(),
            Default::default(),
            Default::default(),
        )?;
    }

    for block in blocks {
        let block_id = BlockHashOrNumber::Hash(block.block.header.hash);
        let expected_block = block.block.unseal();

        let actual_block = provider.block(block_id)?;
        let actual_block_txs = provider.transactions_by_block(block_id)?;
        let actual_block_tx_count = provider.transaction_count_by_block(block_id)?;

        assert_eq!(actual_block_tx_count, Some(expected_block.body.len() as u64));
        assert_eq!(actual_block_txs, Some(expected_block.body.clone()));
        assert_eq!(actual_block, Some(expected_block));
    }

    Ok(())
}
