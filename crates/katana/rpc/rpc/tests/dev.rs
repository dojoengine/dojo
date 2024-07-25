#![allow(deprecated)]

use std::sync::Arc;

use alloy_primitives::U256;
use katana_core::backend::config::StarknetConfig;
use katana_core::backend::Backend;
use katana_core::sequencer::SequencerConfig;
use katana_executor::implementation::blockifier::BlockifierFactory;
use katana_primitives::genesis::allocation::DevAllocationsGenerator;
use katana_primitives::genesis::constant::DEFAULT_PREFUNDED_ACCOUNT_BALANCE;
use katana_primitives::genesis::Genesis;
use katana_provider::traits::block::{BlockNumberProvider, BlockProvider};
use katana_provider::traits::env::BlockEnvProvider;
use katana_rpc::dev::DevApi;

fn create_test_sequencer_config() -> (SequencerConfig, StarknetConfig) {
    let accounts = DevAllocationsGenerator::new(2)
        .with_balance(U256::from(DEFAULT_PREFUNDED_ACCOUNT_BALANCE))
        .generate();

    let mut genesis = Genesis::default();
    genesis.extend_allocations(accounts.into_iter().map(|(k, v)| (k, v.into())));

    (
        SequencerConfig::default(),
        StarknetConfig { genesis, disable_fee: true, ..Default::default() },
    )
}

async fn create_test_dev_api() -> (DevApi<BlockifierFactory>, Arc<Backend<BlockifierFactory>>) {
    let (sequencer_config, starknet_config) = create_test_sequencer_config();
    let (_, backend, bp) =
        katana_core::build_node_components(sequencer_config, starknet_config).await.unwrap();
    (DevApi::new(backend.clone(), bp), backend)
}

#[tokio::test]
async fn test_next_block_timestamp_in_past() {
    let (api, backend) = create_test_dev_api().await;
    let provider = backend.blockchain.provider();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    backend.update_block_env(&mut block_env);

    let block1 = backend.mine_empty_block(&block_env).unwrap().block_number;
    let block1_timestamp = provider.block(block1.into()).unwrap().unwrap().header.timestamp;
    api.set_next_block_timestamp(block1_timestamp - 1000).unwrap();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    backend.update_block_env(&mut block_env);

    let block2 = backend.mine_empty_block(&block_env).unwrap().block_number;
    let block2_timestamp = provider.block(block2.into()).unwrap().unwrap().header.timestamp;

    assert_eq!(block2_timestamp, block1_timestamp - 1000, "timestamp should be updated");
}

#[tokio::test]
async fn test_set_next_block_timestamp_in_future() {
    let (api, backend) = create_test_dev_api().await;
    let provider = backend.blockchain.provider();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    backend.update_block_env(&mut block_env);
    let block1 = backend.mine_empty_block(&block_env).unwrap().block_number;

    let block1_timestamp = provider.block(block1.into()).unwrap().unwrap().header.timestamp;

    api.set_next_block_timestamp(block1_timestamp + 1000).unwrap();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    backend.update_block_env(&mut block_env);
    let block2 = backend.mine_empty_block(&block_env).unwrap().block_number;

    let block2_timestamp = provider.block(block2.into()).unwrap().unwrap().header.timestamp;

    assert_eq!(block2_timestamp, block1_timestamp + 1000, "timestamp should be updated");
}
#[tokio::test]
async fn test_increase_next_block_timestamp() {
    let (api, backend) = create_test_dev_api().await;
    let provider = backend.blockchain.provider();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    backend.update_block_env(&mut block_env);
    let block1 = backend.mine_empty_block(&block_env).unwrap().block_number;

    let block1_timestamp = provider.block(block1.into()).unwrap().unwrap().header.timestamp;

    api.increase_next_block_timestamp(1000).unwrap();

    let block_num = provider.latest_number().unwrap();
    let mut block_env = provider.block_env_at(block_num.into()).unwrap().unwrap();
    backend.update_block_env(&mut block_env);
    let block2 = backend.mine_empty_block(&block_env).unwrap().block_number;

    let block2_timestamp = provider.block(block2.into()).unwrap().unwrap().header.timestamp;

    // Depending on the current time and the machine we run on, we may have 1 sec difference
    // between the expected and actual timestamp.
    // We take this possible delay in account to have the test more robust for now,
    // but it may due to how the timestamp is updated in the sequencer.
    assert!(
        block2_timestamp == block1_timestamp + 1000 || block2_timestamp == block1_timestamp + 1001,
        "timestamp should be updated"
    );
}

// #[tokio::test]
// async fn test_set_storage_at_on_instant_mode() {
//     let sequencer = create_test_sequencer().await;
//     sequencer.backend().mine_empty_block();

//     let contract_address = ContractAddress(patricia_key!("0x1337"));
//     let key = StorageKey(patricia_key!("0x20"));
//     let val = stark_felt!("0xABC");

//     {
//         let mut state = sequencer.backend().state.write().await;
//         let read_val = state.get_storage_at(contract_address, key).unwrap();
//         assert_eq!(stark_felt!("0x0"), read_val, "latest storage value should be 0");
//     }

//     sequencer.set_storage_at(contract_address, key, val).await.unwrap();

//     {
//         let mut state = sequencer.backend().state.write().await;
//         let read_val = state.get_storage_at(contract_address, key).unwrap();
//         assert_eq!(val, read_val, "latest storage value incorrect after generate");
//     }
// }
