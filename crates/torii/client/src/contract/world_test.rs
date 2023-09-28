use std::time::Duration;

use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_world::manifest::Manifest;
use dojo_world::migration::strategy::prepare_for_migration;
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{Declarable, Deployable};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, FieldElement};

use super::{WorldContract, WorldContractReader};

#[tokio::test(flavor = "multi_thread")]
async fn test_world_contract_reader() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();
    let provider = account.provider();
    let (world_address, executor_address) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../../examples/ecs/target/dev".into()).unwrap(),
    )
    .await;

    let world = WorldContractReader::new(world_address, provider);
    let executor = world.executor(BlockId::Tag(BlockTag::Latest)).await.unwrap();

    assert_eq!(executor, executor_address);
}

pub async fn deploy_world(
    sequencer: &TestSequencer,
    path: Utf8PathBuf,
) -> (FieldElement, FieldElement) {
    let manifest = Manifest::load_from_path(path.join("manifest.json")).unwrap();
    let world = WorldDiff::compute(manifest.clone(), None);
    let account = sequencer.account();

    let strategy = prepare_for_migration(
        None,
        Some(FieldElement::from_hex_be("0x12345").unwrap()),
        path,
        world,
    )
    .unwrap();
    let executor_address = strategy
        .executor
        .unwrap()
        .deploy(manifest.clone().executor.class_hash, vec![], &account, Default::default())
        .await
        .unwrap()
        .contract_address;

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    let world_address = strategy
        .world
        .unwrap()
        .deploy(
            manifest.clone().world.class_hash,
            vec![executor_address],
            &account,
            Default::default(),
        )
        .await
        .unwrap()
        .contract_address;

    let mut declare_output = vec![];
    for model in strategy.models {
        let res = model.declare(&account, Default::default()).await.unwrap();
        declare_output.push(res);
    }

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    let _ = WorldContract::new(world_address, &account)
        .register_models(&declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>())
        .await
        .unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    for contract in strategy.contracts {
        let declare_res = contract.declare(&account, Default::default()).await.unwrap();
        contract
            .deploy(declare_res.class_hash, vec![], &account, Default::default())
            .await
            .unwrap();
    }

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    (world_address, executor_address)
}
