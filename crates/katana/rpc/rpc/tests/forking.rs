use anyhow::Result;
use assert_matches::assert_matches;
use cainome::rs::abigen_legacy;
use dojo_test_utils::sequencer::{get_default_test_config, TestSequencer};
use jsonrpsee::http_client::HttpClientBuilder;
use katana_node::config::fork::ForkingConfig;
use katana_node::config::SequencingConfig;
use katana_primitives::block::{BlockHashOrNumber, BlockIdOrTag, BlockNumber};
use katana_primitives::chain::NamedChainId;
use katana_primitives::genesis::constant::DEFAULT_ETH_FEE_TOKEN_ADDRESS;
use katana_primitives::transaction::TxHash;
use katana_primitives::{felt, Felt};
use katana_rpc_api::dev::DevApiClient;
use starknet::core::types::MaybePendingBlockWithTxs;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use url::Url;

const SEPOLIA_CHAIN_ID: Felt = NamedChainId::SN_SEPOLIA;
const SEPOLIA_URL: &str = "https://api.cartridge.gg/x/starknet/sepolia";
const FORK_BLOCK_NUMBER: BlockNumber = 268_471;

fn forking_cfg() -> ForkingConfig {
    ForkingConfig {
        url: Url::parse(SEPOLIA_URL).unwrap(),
        block: Some(BlockHashOrNumber::Num(FORK_BLOCK_NUMBER)),
    }
}

fn provider(url: Url) -> JsonRpcClient<HttpTransport> {
    JsonRpcClient::new(HttpTransport::new(url))
}

type LocalTestVector = Vec<(BlockNumber, TxHash)>;

async fn forked_sequencer() -> (TestSequencer, impl Provider, LocalTestVector) {
    let mut config = get_default_test_config(SequencingConfig::default());
    config.forking = Some(forking_cfg());

    let sequencer = TestSequencer::start(config).await;
    let provider = provider(sequencer.url());

    let mut txs_vector: LocalTestVector = Vec::new();

    {
        // create some emtpy blocks and dummy transactions
        abigen_legacy!(FeeToken, "crates/katana/rpc/rpc/tests/test_data/erc20.json");
        let contract = FeeToken::new(DEFAULT_ETH_FEE_TOKEN_ADDRESS.into(), sequencer.account());
        let client = HttpClientBuilder::default().build(sequencer.url()).unwrap();

        for i in 0..10 {
            let amount = Uint256 { low: Felt::ONE, high: Felt::ZERO };
            let res = contract.transfer(&Felt::ONE, &amount).send().await.unwrap();
            client.generate_block().await.expect("failed to create block");
            txs_vector.push((FORK_BLOCK_NUMBER + i, res.transaction_hash));
        }
    }

    (sequencer, provider, txs_vector)
}

#[tokio::test]
async fn can_fork() -> Result<()> {
    let (_sequencer, provider, _) = forked_sequencer().await;

    let block = provider.block_number().await?;
    let chain = provider.chain_id().await?;

    assert_eq!(chain, SEPOLIA_CHAIN_ID);
    assert_eq!(block, FORK_BLOCK_NUMBER + 10);

    Ok(())
}

async fn assert_get_block_methods(provider: &impl Provider, num: BlockNumber) -> Result<()> {
    let id = BlockIdOrTag::Number(num);

    let block = provider.get_block_with_txs(id).await?;
    assert_matches!(block, MaybePendingBlockWithTxs::Block(b) if b.block_number == num);

    let block = provider.get_block_with_receipts(id).await?;
    assert_matches!(block, starknet::core::types::MaybePendingBlockWithReceipts::Block(b) if b.block_number == num);

    let block = provider.get_block_with_tx_hashes(id).await?;
    assert_matches!(block, starknet::core::types::MaybePendingBlockWithTxHashes::Block(b) if b.block_number == num);

    let result = provider.get_block_transaction_count(id).await;
    assert!(result.is_ok());

    let state = provider.get_state_update(id).await?;
    assert_matches!(state, starknet::core::types::MaybePendingStateUpdate::Update(_));

    Ok(())
}

#[tokio::test]
async fn forked_blocks() -> Result<()> {
    let (_sequencer, provider, _) = forked_sequencer().await;

    let block_num = FORK_BLOCK_NUMBER;
    assert_get_block_methods(&provider, block_num).await?;

    let block_num = FORK_BLOCK_NUMBER - 5;
    assert_get_block_methods(&provider, block_num).await?;

    let block_num = FORK_BLOCK_NUMBER + 5;
    assert_get_block_methods(&provider, block_num).await?;

    Ok(())
}

async fn assert_get_transaction_methods(provider: &impl Provider, tx_hash: TxHash) -> Result<()> {
    let tx = provider.get_transaction_by_hash(tx_hash).await?;
    assert_eq!(*tx.transaction_hash(), tx_hash);
    let tx = provider.get_transaction_receipt(tx_hash).await?;
    assert_eq!(*tx.receipt.transaction_hash(), tx_hash);
    let result = provider.get_transaction_status(tx_hash).await;
    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn forked_transactions() -> Result<()> {
    let (_sequencer, provider, local_only_data) = forked_sequencer().await;

    // https://sepolia.voyager.online/tx/0x5ae12c42a01c71cff20e1ce5bdeeb07cd2d5ddbc86e1157d4bdf2e71dc2d866
    // transaction in block num 268_533
    let tx_hash = felt!("0x5ae12c42a01c71cff20e1ce5bdeeb07cd2d5ddbc86e1157d4bdf2e71dc2d866");
    assert_get_transaction_methods(&provider, tx_hash).await?;

    // get local only transactions
    for (_, tx_hash) in local_only_data {
        assert_get_transaction_methods(&provider, tx_hash).await?;
    }

    // https://sepolia.voyager.online/tx/0x3e336c36a1cba6f3a69d8b5aeed95f81ea0499edbdf2762d00ff641310ecc10
    // transaction in block num 268_473 (plus 3 after the fork block number)
    let tx_hash = felt!("0x3e336c36a1cba6f3a69d8b5aeed95f81ea0499edbdf2762d00ff641310ecc10");

    Ok(())
}
