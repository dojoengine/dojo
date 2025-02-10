use anyhow::Result;
use assert_matches::assert_matches;
use cainome::rs::abigen_legacy;
use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use katana_node::config::fork::ForkingConfig;
use katana_node::config::sequencing::SequencingConfig;
use katana_primitives::block::{BlockHash, BlockHashOrNumber, BlockIdOrTag, BlockNumber, BlockTag};
use katana_primitives::chain::NamedChainId;
use katana_primitives::event::MaybeForkedContinuationToken;
use katana_primitives::genesis::constant::DEFAULT_ETH_FEE_TOKEN_ADDRESS;
use katana_primitives::transaction::TxHash;
use katana_primitives::{felt, Felt};
use starknet::core::types::{EventFilter, MaybePendingBlockWithTxHashes, StarknetError};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use url::Url;

mod common;

const SEPOLIA_CHAIN_ID: Felt = NamedChainId::SN_SEPOLIA;
const SEPOLIA_URL: &str = "https://api.cartridge.gg/x/starknet/sepolia";
const FORK_BLOCK_NUMBER: BlockNumber = 268_471;
const FORK_BLOCK_HASH: BlockHash =
    felt!("0x208950cfcbba73ecbda1c14e4d58d66a8d60655ea1b9dcf07c16014ae8a93cd");

fn forking_cfg() -> ForkingConfig {
    ForkingConfig {
        url: Url::parse(SEPOLIA_URL).unwrap(),
        block: Some(BlockHashOrNumber::Num(FORK_BLOCK_NUMBER)),
    }
}

type LocalTestVector = Vec<((BlockNumber, BlockHash), TxHash)>;

/// A helper function for setting a test environment, forked from the SN_SEPOLIA chain.
/// This function will forked Sepolia at block [`FORK_BLOCK_NUMBER`] and create 10 blocks, each has
/// a single transaction.
///
/// The returned [`TestVector`] is a list of all the locally created blocks and transactions.
async fn setup_test_inner(no_mining: bool) -> (TestSequencer, impl Provider, LocalTestVector) {
    let mut config = get_default_test_config(SequencingConfig::default());
    config.sequencing.no_mining = no_mining;
    config.forking = Some(forking_cfg());

    let sequencer = TestSequencer::start(config).await;
    let provider = JsonRpcClient::new(HttpTransport::new(sequencer.url()));

    let mut txs_vector: LocalTestVector = Vec::new();

    // create some emtpy blocks and dummy transactions
    abigen_legacy!(FeeToken, "crates/katana/rpc/rpc/tests/test_data/erc20.json");
    let contract = FeeToken::new(DEFAULT_ETH_FEE_TOKEN_ADDRESS.into(), sequencer.account());

    if no_mining {
        // In no mining mode, bcs we're not producing any blocks, the transactions that we send
        // will all be included in the same block (pending).
        for _ in 1..=10 {
            let amount = Uint256 { low: Felt::ONE, high: Felt::ZERO };
            let res = contract.transfer(&Felt::ONE, &amount).send().await.unwrap();
            dojo_utils::TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

            // events in pending block doesn't have block hash and number, so we can safely put
            // dummy values here.
            txs_vector.push(((0, Felt::ZERO), res.transaction_hash));
        }
    } else {
        // We're in auto mining, each transaction will create a new block
        for i in 1..=10 {
            let amount = Uint256 { low: Felt::ONE, high: Felt::ZERO };
            let res = contract.transfer(&Felt::ONE, &amount).send().await.unwrap();
            let _ =
                dojo_utils::TransactionWaiter::new(res.transaction_hash, &provider).await.unwrap();

            let block_num = FORK_BLOCK_NUMBER + i;

            let block_id = BlockIdOrTag::Number(block_num);
            let block = provider.get_block_with_tx_hashes(block_id).await.unwrap();
            let block_hash = match block {
                MaybePendingBlockWithTxHashes::Block(b) => b.block_hash,
                _ => panic!("Expected a block"),
            };

            txs_vector.push(((FORK_BLOCK_NUMBER + i, block_hash), res.transaction_hash));
        }
    }

    (sequencer, provider, txs_vector)
}

async fn setup_test() -> (TestSequencer, impl Provider, LocalTestVector) {
    setup_test_inner(false).await
}

async fn setup_test_pending() -> (TestSequencer, impl Provider, LocalTestVector) {
    setup_test_inner(true).await
}

#[tokio::test]
async fn can_fork() -> Result<()> {
    let (_sequencer, provider, _) = setup_test().await;

    let block = provider.block_number().await?;
    let chain = provider.chain_id().await?;

    assert_eq!(chain, SEPOLIA_CHAIN_ID);
    assert_eq!(block, FORK_BLOCK_NUMBER + 10);

    Ok(())
}

#[tokio::test]
async fn get_blocks_from_num() -> Result<()> {
    use starknet::core::types::{
        MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    };

    let (_sequencer, provider, local_only_block) = setup_test().await;

    // -----------------------------------------------------------------------
    // Get the forked block
    // https://sepolia.voyager.online/block/0x208950cfcbba73ecbda1c14e4d58d66a8d60655ea1b9dcf07c16014ae8a93cd

    let num = FORK_BLOCK_NUMBER; // 268471
    let id = BlockIdOrTag::Number(num);

    let block = provider.get_block_with_txs(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxs::Block(b) if b.block_number == num);

    let block = provider.get_block_with_receipts(id).await?;
    assert_matches!(block, MaybePendingBlockWithReceipts::Block(b) if b.block_number == num);

    let block = provider.get_block_with_tx_hashes(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxHashes::Block(b) if b.block_number == num);

    let result = provider.get_block_transaction_count(id).await;
    assert!(result.is_ok());

    // TODO: uncomment this once we include genesis forked state update
    // let state = provider.get_state_update(id).await?;
    // assert_matches!(state, starknet::core::types::MaybePendingStateUpdate::Update(_));

    // -----------------------------------------------------------------------
    // Get a block before the forked block

    // https://sepolia.voyager.online/block/0x42dc67af5003d212ac6cd784e72db945ea4d619898f30f422358ff215cbe1e4
    let num = FORK_BLOCK_NUMBER - 5; // 268466
    let id = BlockIdOrTag::Number(num);

    let block = provider.get_block_with_txs(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxs::Block(b) if b.block_number == num);

    let block = provider.get_block_with_receipts(id).await?;
    assert_matches!(block, MaybePendingBlockWithReceipts::Block(b) if b.block_number == num);

    let block = provider.get_block_with_tx_hashes(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxHashes::Block(b) if b.block_number == num);

    let result = provider.get_block_transaction_count(id).await;
    assert!(result.is_ok());

    // TODO: uncomment this once we include genesis forked state update
    // let state = provider.get_state_update(id).await?;
    // assert_matches!(state, starknet::core::types::MaybePendingStateUpdate::Update(_));

    // -----------------------------------------------------------------------
    // Get a block that is locally generated

    for ((num, _), _) in local_only_block {
        let id = BlockIdOrTag::Number(num);

        let block = provider.get_block_with_txs(id).await?;
        assert_matches!(block, MaybePendingBlockWithTxs::Block(b) if b.block_number == num);

        let block = provider.get_block_with_receipts(id).await?;
        assert_matches!(block, starknet::core::types::MaybePendingBlockWithReceipts::Block(b) if b.block_number == num);

        let block = provider.get_block_with_tx_hashes(id).await?;
        assert_matches!(block, starknet::core::types::MaybePendingBlockWithTxHashes::Block(b) if b.block_number == num);

        let count = provider.get_block_transaction_count(id).await?;
        assert_eq!(count, 1, "all the locally generated blocks should have 1 tx");

        // TODO: uncomment this once we include genesis forked state update
        // let state = provider.get_state_update(id).await?;
        // assert_matches!(state, starknet::core::types::MaybePendingStateUpdate::Update(_));
    }

    // -----------------------------------------------------------------------
    // Get a block that only exist in the forked chain

    // https://sepolia.voyager.online/block/0x347a9fa25700e7a2d8f26b39c0ecf765be9a78c559b9cae722a659f25182d10
    // We only created 10 local blocks so this is fine.
    let id = BlockIdOrTag::Number(270_328);
    let result = provider.get_block_with_txs(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_with_receipts(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_with_tx_hashes(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_transaction_count(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_state_update(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    // -----------------------------------------------------------------------
    // Get block that doesn't exist on the both the forked and local chain

    let id = BlockIdOrTag::Number(i64::MAX as u64);
    let result = provider.get_block_with_txs(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_with_receipts(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_with_tx_hashes(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_transaction_count(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_state_update(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    Ok(())
}

#[tokio::test]
async fn get_blocks_from_hash() -> Result<()> {
    use starknet::core::types::{
        MaybePendingBlockWithReceipts, MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs,
    };

    let (_sequencer, provider, local_only_block) = setup_test().await;

    // -----------------------------------------------------------------------
    // Get the forked block

    // https://sepolia.voyager.online/block/0x208950cfcbba73ecbda1c14e4d58d66a8d60655ea1b9dcf07c16014ae8a93cd
    let hash = felt!("0x208950cfcbba73ecbda1c14e4d58d66a8d60655ea1b9dcf07c16014ae8a93cd"); // 268471
    let id = BlockIdOrTag::Hash(hash);

    let block = provider.get_block_with_txs(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxs::Block(b) if b.block_hash == hash);

    let block = provider.get_block_with_receipts(id).await?;
    assert_matches!(block, MaybePendingBlockWithReceipts::Block(b) if b.block_hash == hash);

    let block = provider.get_block_with_tx_hashes(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxHashes::Block(b) if b.block_hash == hash);

    let result = provider.get_block_transaction_count(id).await;
    assert!(result.is_ok());

    // TODO: uncomment this once we include genesis forked state update
    // let state = provider.get_state_update(id).await?;
    // assert_matches!(state, starknet::core::types::MaybePendingStateUpdate::Update(_));

    // -----------------------------------------------------------------------
    // Get a block before the forked block
    // https://sepolia.voyager.online/block/0x42dc67af5003d212ac6cd784e72db945ea4d619898f30f422358ff215cbe1e4

    let hash = felt!("0x42dc67af5003d212ac6cd784e72db945ea4d619898f30f422358ff215cbe1e4"); // 268466
    let id = BlockIdOrTag::Hash(hash);

    let block = provider.get_block_with_txs(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxs::Block(b) if b.block_hash == hash);

    let block = provider.get_block_with_receipts(id).await?;
    assert_matches!(block, MaybePendingBlockWithReceipts::Block(b) if b.block_hash == hash);

    let block = provider.get_block_with_tx_hashes(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxHashes::Block(b) if b.block_hash == hash);

    let result = provider.get_block_transaction_count(id).await;
    assert!(result.is_ok());

    // TODO: uncomment this once we include genesis forked state update
    // let state = provider.get_state_update(id).await?;
    // assert_matches!(state, starknet::core::types::MaybePendingStateUpdate::Update(_));

    // -----------------------------------------------------------------------
    // Get a block that is locally generated

    for ((_, hash), _) in local_only_block {
        let id = BlockIdOrTag::Hash(hash);

        let block = provider.get_block_with_txs(id).await?;
        assert_matches!(block, MaybePendingBlockWithTxs::Block(b) if b.block_hash == hash);

        let block = provider.get_block_with_receipts(id).await?;
        assert_matches!(block, MaybePendingBlockWithReceipts::Block(b) if b.block_hash == hash);

        let block = provider.get_block_with_tx_hashes(id).await?;
        assert_matches!(block, MaybePendingBlockWithTxHashes::Block(b) if b.block_hash == hash);

        let result = provider.get_block_transaction_count(id).await;
        assert!(result.is_ok());

        // TODO: uncomment this once we include genesis forked state update
        // let state = provider.get_state_update(id).await?;
        // assert_matches!(state, starknet::core::types::MaybePendingStateUpdate::Update(_));
    }

    // -----------------------------------------------------------------------
    // Get a block that only exist in the forked chain

    // https://sepolia.voyager.online/block/0x347a9fa25700e7a2d8f26b39c0ecf765be9a78c559b9cae722a659f25182d10
    // We only created 10 local blocks so this is fine.
    let id = BlockIdOrTag::Number(270_328);
    let result = provider.get_block_with_txs(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_with_receipts(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_with_tx_hashes(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_transaction_count(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_state_update(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    // -----------------------------------------------------------------------
    // Get block that doesn't exist on the both the forked and local chain

    let id = BlockIdOrTag::Number(i64::MAX as u64);
    let result = provider.get_block_with_txs(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_with_receipts(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_with_tx_hashes(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_block_transaction_count(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    let result = provider.get_state_update(id).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    Ok(())
}

#[tokio::test]
async fn get_transactions() -> Result<()> {
    let (_sequencer, provider, local_only_data) = setup_test().await;

    // -----------------------------------------------------------------------
    // Get txs before the forked block.

    // https://sepolia.voyager.online/tx/0x81207d4244596678e186f6ab9c833fe40a4b35291e8a90b9a163f7f643df9f
    // Transaction in block num FORK_BLOCK_NUMBER - 1
    let tx_hash = felt!("0x81207d4244596678e186f6ab9c833fe40a4b35291e8a90b9a163f7f643df9f");

    let tx = provider.get_transaction_by_hash(tx_hash).await?;
    assert_eq!(*tx.transaction_hash(), tx_hash);

    let tx = provider.get_transaction_receipt(tx_hash).await?;
    assert_eq!(*tx.receipt.transaction_hash(), tx_hash);

    let result = provider.get_transaction_status(tx_hash).await;
    assert!(result.is_ok());

    // https://sepolia.voyager.online/tx/0x1b18d62544d4ef749befadabcec019d83218d3905abd321b4c1b1fc948d5710
    // Transaction in block num FORK_BLOCK_NUMBER - 2
    let tx_hash = felt!("0x1b18d62544d4ef749befadabcec019d83218d3905abd321b4c1b1fc948d5710");

    let tx = provider.get_transaction_by_hash(tx_hash).await?;
    assert_eq!(*tx.transaction_hash(), tx_hash);

    let tx = provider.get_transaction_receipt(tx_hash).await?;
    assert_eq!(*tx.receipt.transaction_hash(), tx_hash);

    let result = provider.get_transaction_status(tx_hash).await;
    assert!(result.is_ok());

    // -----------------------------------------------------------------------
    // Get the locally created transactions.

    for (_, tx_hash) in local_only_data {
        let tx = provider.get_transaction_by_hash(tx_hash).await?;
        assert_eq!(*tx.transaction_hash(), tx_hash);

        let tx = provider.get_transaction_receipt(tx_hash).await?;
        assert_eq!(*tx.receipt.transaction_hash(), tx_hash);

        let result = provider.get_transaction_status(tx_hash).await;
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Get a tx that exists in the forked chain but is included in a block past the forked block.

    // https://sepolia.voyager.online/block/0x335a605f2c91873f8f830a6e5285e704caec18503ca28c18485ea6f682eb65e
    // transaction in block num 268,474 (FORK_BLOCK_NUMBER + 3)
    let tx_hash = felt!("0x335a605f2c91873f8f830a6e5285e704caec18503ca28c18485ea6f682eb65e");
    let result = provider.get_transaction_by_hash(tx_hash).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::TransactionHashNotFound);

    let result = provider.get_transaction_receipt(tx_hash).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::TransactionHashNotFound);

    let result = provider.get_transaction_status(tx_hash).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::TransactionHashNotFound);

    Ok(())
}

#[tokio::test]
#[rstest::rstest]
#[case(BlockIdOrTag::Number(FORK_BLOCK_NUMBER))]
#[case(BlockIdOrTag::Hash(felt!("0x208950cfcbba73ecbda1c14e4d58d66a8d60655ea1b9dcf07c16014ae8a93cd")))]
async fn get_events_partially_from_forked(#[case] block_id: BlockIdOrTag) -> Result<()> {
    let (_sequencer, provider, _) = setup_test().await;
    let forked_provider = JsonRpcClient::new(HttpTransport::new(Url::parse(SEPOLIA_URL)?));

    // -----------------------------------------------------------------------
    // Fetch events partially from forked block.
    //
    // Here we want to make sure the continuation token is working as expected.

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: Some(block_id),
        from_block: Some(block_id),
    };

    // events fetched directly from the forked chain.
    let result = forked_provider.get_events(filter.clone(), None, 5).await?;
    let events = result.events;

    // events fetched through the forked katana.
    let result = provider.get_events(filter, None, 5).await?;
    let forked_events = result.events;

    let token = MaybeForkedContinuationToken::parse(&result.continuation_token.unwrap())?;
    assert_matches!(token, MaybeForkedContinuationToken::Forked(_));

    for (a, b) in events.iter().zip(forked_events) {
        assert_eq!(a.block_number, Some(FORK_BLOCK_NUMBER));
        assert_eq!(a.block_hash, Some(FORK_BLOCK_HASH));
        assert_eq!(a.block_number, b.block_number);
        assert_eq!(a.block_hash, b.block_hash);
        assert_eq!(a.transaction_hash, b.transaction_hash);
        assert_eq!(a.from_address, b.from_address);
        assert_eq!(a.keys, b.keys);
        assert_eq!(a.data, b.data);
    }

    Ok(())
}

#[tokio::test]
#[rstest::rstest]
#[case(BlockIdOrTag::Number(FORK_BLOCK_NUMBER))]
#[case(BlockIdOrTag::Hash(felt!("0x208950cfcbba73ecbda1c14e4d58d66a8d60655ea1b9dcf07c16014ae8a93cd")))]
async fn get_events_all_from_forked(#[case] block_id: BlockIdOrTag) -> Result<()> {
    let (_sequencer, provider, _) = setup_test().await;
    let forked_provider = JsonRpcClient::new(HttpTransport::new(Url::parse(SEPOLIA_URL)?));

    // -----------------------------------------------------------------------
    // Fetch events from the forked block (ie `FORK_BLOCK_NUMBER`) only.
    //
    // Based on https://sepolia.voyager.online/block/0x208950cfcbba73ecbda1c14e4d58d66a8d60655ea1b9dcf07c16014ae8a93cd, there are only 89 events in the `FORK_BLOCK_NUMBER` block.
    // So we set the chunk size to 100 to ensure we get all the events in one request.

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: Some(block_id),
        from_block: Some(block_id),
    };

    // events fetched directly from the forked chain.
    let result = forked_provider.get_events(filter.clone(), None, 100).await?;
    let events = result.events;

    // events fetched through the forked katana.
    let result = provider.get_events(filter, None, 100).await?;
    let forked_events = result.events;

    assert!(result.continuation_token.is_none());

    for (a, b) in events.iter().zip(forked_events) {
        assert_eq!(a.block_number, Some(FORK_BLOCK_NUMBER));
        assert_eq!(a.block_hash, Some(FORK_BLOCK_HASH));
        assert_eq!(a.block_number, b.block_number);
        assert_eq!(a.block_hash, b.block_hash);
        assert_eq!(a.transaction_hash, b.transaction_hash);
        assert_eq!(a.from_address, b.from_address);
        assert_eq!(a.keys, b.keys);
        assert_eq!(a.data, b.data);
    }

    Ok(())
}

#[tokio::test]
async fn get_events_local() -> Result<()> {
    let (_sequencer, provider, local_only_data) = setup_test().await;

    // -----------------------------------------------------------------------
    // Get events from the local chain block.

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: None,
        from_block: Some(BlockIdOrTag::Number(FORK_BLOCK_NUMBER + 1)),
    };

    let result = provider.get_events(filter, None, 10).await?;
    let forked_events = result.events;

    // compare the events

    for (event, (block, tx)) in forked_events.iter().zip(local_only_data.iter()) {
        let (block_number, block_hash) = block;

        assert_eq!(event.transaction_hash, *tx);
        assert_eq!(event.block_hash, Some(*block_hash));
        assert_eq!(event.block_number, Some(*block_number));
    }

    Ok(())
}

#[tokio::test]
async fn get_events_pending_exhaustive() -> Result<()> {
    let (_sequencer, provider, local_only_data) = setup_test_pending().await;

    // -----------------------------------------------------------------------
    // Get events from the local chain pending block.

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: Some(BlockIdOrTag::Tag(BlockTag::Pending)),
        from_block: Some(BlockIdOrTag::Tag(BlockTag::Pending)),
    };

    let result = provider.get_events(filter, None, 10).await?;
    let events = result.events;

    // This is expected behaviour, as the pending block is not yet closed.
    // so there may still more events to come.
    assert!(result.continuation_token.is_some());

    for (event, (_, tx)) in events.iter().zip(local_only_data.iter()) {
        assert_eq!(event.transaction_hash, *tx);
        // pending events should not have block number and block hash.
        assert_eq!(event.block_hash, None);
        assert_eq!(event.block_number, None);
    }

    Ok(())
}

#[tokio::test]
#[rstest::rstest]
#[case(BlockIdOrTag::Number(FORK_BLOCK_NUMBER))]
#[case(BlockIdOrTag::Hash(felt!("0x208950cfcbba73ecbda1c14e4d58d66a8d60655ea1b9dcf07c16014ae8a93cd")))] // FORK_BLOCK_NUMBER hash
async fn get_events_forked_and_local_boundary_exhaustive(
    #[case] block_id: BlockIdOrTag,
) -> Result<()> {
    let (_sequencer, provider, local_only_data) = setup_test().await;
    let forked_provider = JsonRpcClient::new(HttpTransport::new(Url::parse(SEPOLIA_URL)?));

    // -----------------------------------------------------------------------
    // Get events from that cross the boundaries between forked and local chain block.
    //
    // Total events in `FORK_BLOCK_NUMBER` block is 89. While `FORK_BLOCK_NUMBER` + 1 is 1 ∴ 89 + 1
    // = 90 events.

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: Some(block_id),
        from_block: Some(block_id),
    };

    // events fetched directly from the forked chain.
    let result = forked_provider.get_events(filter.clone(), None, 100).await?;
    let events = result.events;

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: Some(BlockIdOrTag::Tag(BlockTag::Latest)),
        from_block: Some(block_id),
    };

    let result = provider.get_events(filter, None, 100).await?;
    let boundary_events = result.events;

    // because we're pointing to latest block, we should not have anymore continuation token.
    assert!(result.continuation_token.is_none());

    let forked_events = &boundary_events[..89];
    let local_events = &boundary_events[89..];

    similar_asserts::assert_eq!(forked_events, events);

    for (event, (block, tx)) in local_events.iter().zip(local_only_data.iter()) {
        let (block_number, block_hash) = block;

        assert_eq!(event.transaction_hash, *tx);
        assert_eq!(event.block_hash, Some(*block_hash));
        assert_eq!(event.block_number, Some(*block_number));
    }

    Ok(())
}

#[tokio::test]
#[rstest::rstest]
#[case(BlockIdOrTag::Number(FORK_BLOCK_NUMBER - 1))]
#[case(BlockIdOrTag::Hash(felt!("0x4a6a79bfefceb03af4f78758785b0c40ddf9f757e9a8f72f01ecb0aad11e298")))] // FORK_BLOCK_NUMBER - 1 hash
async fn get_events_forked_and_local_boundary_non_exhaustive(
    #[case] block_id: BlockIdOrTag,
) -> Result<()> {
    let (_sequencer, provider, _) = setup_test().await;
    let forked_provider = JsonRpcClient::new(HttpTransport::new(Url::parse(SEPOLIA_URL)?));

    // -----------------------------------------------------------------------
    // Get events that cross the boundaries between forked and local chain block, but
    // not all events from the forked range is fetched.

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: Some(block_id),
        from_block: Some(block_id),
    };

    // events fetched directly from the forked chain.
    let result = forked_provider.get_events(filter.clone(), None, 50).await?;
    let forked_events = result.events;

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: Some(BlockIdOrTag::Tag(BlockTag::Pending)),
        from_block: Some(block_id),
    };

    let result = provider.get_events(filter, None, 50).await?;
    let katana_events = result.events;

    let token = MaybeForkedContinuationToken::parse(&result.continuation_token.unwrap())?;
    assert_matches!(token, MaybeForkedContinuationToken::Forked(_));
    similar_asserts::assert_eq!(katana_events, forked_events);

    Ok(())
}

#[tokio::test]
#[rstest::rstest]
#[case::doesnt_exist_at_all(felt!("0x123"))]
#[case::after_forked_block_but_on_the_forked_chain(felt!("0x21f4c20f9cc721dbaee2eaf44c79342b37c60f55ac37c13a4bdd6785ac2a5e5"))]
async fn get_events_with_invalid_block_hash(#[case] hash: BlockHash) -> Result<()> {
    let (_sequencer, provider, _) = setup_test().await;

    let filter = EventFilter {
        keys: None,
        address: None,
        to_block: Some(BlockIdOrTag::Hash(hash)),
        from_block: Some(BlockIdOrTag::Hash(hash)),
    };

    let result = provider.get_events(filter.clone(), None, 5).await.unwrap_err();
    assert_provider_starknet_err!(result, StarknetError::BlockNotFound);

    Ok(())
}
