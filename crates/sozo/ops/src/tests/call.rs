use dojo_world::contracts::WorldContractReader;
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::Felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;

use crate::test_utils::setup;
use crate::{call, utils};

const CONTRACT_TAG: &str = "dojo_examples-actions";
const ENTRYPOINT: &str = "get_player_position";

#[tokio::test]
async fn call_with_bad_address() {
    let config = KatanaRunnerConfig::default().with_db_dir("/tmp/test-db");
    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(
        call::call(
            world_reader,
            "0xBadCoffeeBadCode".to_string(),
            ENTRYPOINT.to_string(),
            vec![Felt::ZERO, Felt::ZERO],
            None
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn call_with_bad_name() {
    let config = KatanaRunnerConfig::default().with_db_dir("/tmp/test-db");
    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(
        call::call(
            world_reader,
            "BadName".to_string(),
            ENTRYPOINT.to_string(),
            vec![Felt::ZERO, Felt::ZERO],
            None
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn call_with_bad_entrypoint() {
    let config = KatanaRunnerConfig::default().with_db_dir("/tmp/test-db");
    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(
        call::call(
            world_reader,
            CONTRACT_TAG.to_string(),
            "BadEntryPoint".to_string(),
            vec![Felt::ZERO, Felt::ZERO],
            None
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn call_with_bad_calldata() {
    let config = KatanaRunnerConfig::default().with_db_dir("/tmp/test-db");
    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(
        call::call(
            world_reader,
            CONTRACT_TAG.to_string(),
            ENTRYPOINT.to_string(),
            vec![Felt::ZERO],
            None
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn call_with_contract_name() {
    let config = KatanaRunnerConfig::default().with_db_dir("/tmp/test-db");
    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let r =
        call::call(world_reader, CONTRACT_TAG.to_string(), ENTRYPOINT.to_string(), vec![], None)
            .await;

    assert!(r.is_ok());
}

#[tokio::test]
async fn call_with_contract_address() {
    let config = KatanaRunnerConfig::default().with_db_dir("/tmp/test-db");
    let sequencer = KatanaRunner::new_with_config(config).expect("Failed to start runner.");

    let world = setup::setup_with_world(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let contract_address = utils::get_contract_address::<
        SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    >(&world, CONTRACT_TAG)
    .await
    .unwrap();

    assert!(
        call::call(
            world_reader,
            format!("{:#x}", contract_address),
            ENTRYPOINT.to_string(),
            vec![],
            None,
        )
        .await
        .is_ok()
    );
}
