use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::backend::Backend;
use katana_provider::traits::block::{BlockNumberProvider, BlockProvider};
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
    let backend = create_test_backend().await;

    let provider = backend.blockchain.provider();

    assert_eq!(BlockNumberProvider::latest_number(provider).unwrap(), 0);

    backend.mine_empty_block();
    backend.mine_empty_block();

    assert_eq!(BlockNumberProvider::latest_number(provider).unwrap(), 2);
    assert_eq!(backend.env.read().block.block_number, BlockNumber(2));

    let block0 = BlockProvider::block_by_number(provider, 0).unwrap().unwrap();
    let block1 = BlockProvider::block_by_number(provider, 1).unwrap().unwrap();
    let block2 = BlockProvider::block_by_number(provider, 2).unwrap().unwrap();

    assert_eq!(block0.header.number, 0);
    assert_eq!(block1.header.number, 1);
    assert_eq!(block2.header.number, 2);
}
