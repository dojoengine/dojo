use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::backend::Backend;
use starknet_api::block::BlockNumber;

fn create_test_starknet_config() -> StarknetConfig {
    StarknetConfig {
        seed: [0u8; 32],
        total_accounts: 2,
        disable_fee: true,
        env: Environment::default(),
        ..Default::default()
    }
}

async fn create_test_backend() -> Backend {
    Backend::new(create_test_starknet_config()).await
}

#[tokio::test]
async fn test_creating_blocks() {
    let starknet = create_test_backend().await;

    assert_eq!(starknet.blockchain.storage.read().blocks.len(), 1);
    assert_eq!(starknet.blockchain.storage.read().latest_number, 0);

    starknet.mine_empty_block().await;
    starknet.mine_empty_block().await;

    assert_eq!(starknet.blockchain.storage.read().blocks.len(), 3);
    assert_eq!(starknet.blockchain.storage.read().latest_number, 2);
    assert_eq!(starknet.env.read().block.block_number, BlockNumber(2),);

    let block0 = starknet.blockchain.storage.read().block_by_number(0).unwrap().clone();
    let block1 = starknet.blockchain.storage.read().block_by_number(1).unwrap().clone();

    assert_eq!(block0.header.number, 0);
    assert_eq!(block1.header.number, 1);
}
