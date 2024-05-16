use dojo_world::contracts::WorldContractReader;
use katana_runner::KatanaRunner;
use starknet::accounts::SingleOwnerAccount;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::LocalWallet;
use starknet_crypto::FieldElement;

use super::setup;
use crate::{call, utils};

const CONTRACT_NAME: &str = "dojo_examples::actions::actions";
const ENTRYPOINT: &str = "tile_terrain";

#[tokio::test]
async fn call_with_bad_address() {
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(call::call(
        world_reader,
        "0xBadCoffeeBadCode".to_string(),
        ENTRYPOINT.to_string(),
        vec![FieldElement::ZERO, FieldElement::ZERO],
        None
    )
    .await
    .is_err());
}

#[tokio::test]
async fn call_with_bad_name() {
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(call::call(
        world_reader,
        "BadName".to_string(),
        ENTRYPOINT.to_string(),
        vec![FieldElement::ZERO, FieldElement::ZERO],
        None
    )
    .await
    .is_err());
}

#[tokio::test]
async fn call_with_bad_entrypoint() {
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(call::call(
        world_reader,
        CONTRACT_NAME.to_string(),
        "BadEntryPoint".to_string(),
        vec![FieldElement::ZERO, FieldElement::ZERO],
        None
    )
    .await
    .is_err());
}

#[tokio::test]
async fn call_with_bad_calldata() {
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(call::call(
        world_reader,
        CONTRACT_NAME.to_string(),
        ENTRYPOINT.to_string(),
        vec![],
        None
    )
    .await
    .is_err());
}

#[tokio::test]
async fn call_with_contract_name() {
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    assert!(call::call(
        world_reader,
        CONTRACT_NAME.to_string(),
        ENTRYPOINT.to_string(),
        vec![FieldElement::ZERO, FieldElement::ZERO],
        None,
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn call_with_contract_address() {
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();
    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(world.address, provider);

    let contract_address = utils::get_contract_address::<
        SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    >(&world, CONTRACT_NAME.to_string())
    .await
    .unwrap();

    assert!(call::call(
        world_reader,
        format!("{:#x}", contract_address),
        ENTRYPOINT.to_string(),
        vec![FieldElement::ZERO, FieldElement::ZERO],
        None,
    )
    .await
    .is_ok());
}
