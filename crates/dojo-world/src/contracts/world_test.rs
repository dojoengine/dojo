use std::time::Duration;

use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::FieldElement;

use super::{WorldContract, WorldContractReader};
use crate::manifest::Manifest;
use crate::migration::strategy::prepare_for_migration;
use crate::migration::world::WorldDiff;
use crate::migration::{Declarable, Deployable};

#[tokio::test(flavor = "multi_thread")]
async fn test_world_contract_reader() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();
    let provider = account.provider();
    let (world_address, executor_address) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../examples/spawn-and-move/target/dev".into()).unwrap(),
    )
    .await;

    let world = WorldContractReader::new(world_address, provider);
    let executor = world.executor().call().await.unwrap();

    assert_eq!(FieldElement::from(executor), executor_address);
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

    let base_class_hash =
        strategy.base.unwrap().declare(&account, Default::default()).await.unwrap().class_hash;

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    let world_address = strategy
        .world
        .unwrap()
        .deploy(
            manifest.clone().world.class_hash,
            vec![executor_address, base_class_hash],
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

    let world = WorldContract::new(world_address, &account);

    let calls = declare_output
        .iter()
        .map(|o| world.register_model_getcall(&o.class_hash.into()))
        .collect::<Vec<_>>();

    let _ = account.execute(calls).send().await.unwrap();

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    for contract in strategy.contracts {
        let declare_res = contract.declare(&account, Default::default()).await.unwrap();
        contract
            .world_deploy(world_address, declare_res.class_hash, &account, Default::default())
            .await
            .unwrap();
    }

    // wait for the tx to be mined
    tokio::time::sleep(Duration::from_millis(250)).await;

    (world_address, executor_address)
}
