use katana_core::backend::config::{Environment, StarknetConfig};
use katana_core::sequencer::{KatanaSequencer, SequencerConfig};
use katana_provider::traits::block::{BlockNumberProvider, BlockProvider};
use katana_provider::traits::env::BlockEnvProvider;

fn create_test_sequencer_config() -> (SequencerConfig, StarknetConfig) {
    (
        SequencerConfig { block_time: None, ..Default::default() },
        StarknetConfig {
            seed: [0u8; 32],
            total_accounts: 2,
            disable_fee: true,
            env: Environment::default(),
            ..Default::default()
        },
    )
}

async fn create_test_sequencer() -> KatanaSequencer {
    let (sequencer_config, starknet_config) = create_test_sequencer_config();
    KatanaSequencer::new(sequencer_config, starknet_config).await.unwrap()
}

#[tokio::test]
async fn test_next_block_timestamp_in_past() {
    let sequencer = create_test_sequencer().await;
    let provider = sequencer.backend.blockchain.provider();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    sequencer.backend.update_block_env(&mut block_env);
    let block1 = sequencer.backend.mine_empty_block(&block_env).unwrap().block_number;

    let block1_timestamp =
        BlockProvider::block(provider, block1.into()).unwrap().unwrap().header.timestamp;

    sequencer.set_next_block_timestamp(block1_timestamp - 1000).unwrap();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    sequencer.backend.update_block_env(&mut block_env);
    let block2 = sequencer.backend.mine_empty_block(&block_env).unwrap().block_number;

    let block2_timestamp =
        BlockProvider::block(provider, block2.into()).unwrap().unwrap().header.timestamp;

    assert_eq!(block2_timestamp, block1_timestamp - 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_set_next_block_timestamp_in_future() {
    let sequencer = create_test_sequencer().await;
    let provider = sequencer.backend.blockchain.provider();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    sequencer.backend.update_block_env(&mut block_env);
    let block1 = sequencer.backend.mine_empty_block(&block_env).unwrap().block_number;

    let block1_timestamp =
        BlockProvider::block(provider, block1.into()).unwrap().unwrap().header.timestamp;

    sequencer.set_next_block_timestamp(block1_timestamp + 1000).unwrap();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    sequencer.backend.update_block_env(&mut block_env);
    let block2 = sequencer.backend.mine_empty_block(&block_env).unwrap().block_number;

    let block2_timestamp =
        BlockProvider::block(provider, block2.into()).unwrap().unwrap().header.timestamp;

    assert_eq!(block2_timestamp, block1_timestamp + 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_increase_next_block_timestamp() {
    let sequencer = create_test_sequencer().await;
    let provider = sequencer.backend.blockchain.provider();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    sequencer.backend.update_block_env(&mut block_env);
    let block1 = sequencer.backend.mine_empty_block(&block_env).unwrap().block_number;

    let block1_timestamp =
        BlockProvider::block(provider, block1.into()).unwrap().unwrap().header.timestamp;

    sequencer.increase_next_block_timestamp(1000).unwrap();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    sequencer.backend.update_block_env(&mut block_env);
    let block2 = sequencer.backend.mine_empty_block(&block_env).unwrap().block_number;

    let block2_timestamp =
        BlockProvider::block(provider, block2.into()).unwrap().unwrap().header.timestamp;

    assert_eq!(block2_timestamp, block1_timestamp + 1000, "timestamp should be updated");
}

// #[tokio::test]
// async fn test_set_storage_at_on_instant_mode() {
//     let sequencer = create_test_sequencer().await;
//     sequencer.backend.mine_empty_block();

//     let contract_address = ContractAddress(patricia_key!("0x1337"));
//     let key = StorageKey(patricia_key!("0x20"));
//     let val = stark_felt!("0xABC");

//     {
//         let mut state = sequencer.backend.state.write().await;
//         let read_val = state.get_storage_at(contract_address, key).unwrap();
//         assert_eq!(stark_felt!("0x0"), read_val, "latest storage value should be 0");
//     }

//     sequencer.set_storage_at(contract_address, key, val).await.unwrap();

//     {
//         let mut state = sequencer.backend.state.write().await;
//         let read_val = state.get_storage_at(contract_address, key).unwrap();
//         assert_eq!(val, read_val, "latest storage value incorrect after generate");
//     }
// }
