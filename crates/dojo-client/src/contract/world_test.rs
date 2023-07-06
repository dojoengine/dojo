use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{SequencerConfig, TestSequencer};
use dojo_world::manifest::Manifest;
use dojo_world::migration::strategy::prepare_for_migration;
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::{Declarable, Deployable};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, FieldElement};

use super::{WorldContract, WorldContractReader};

#[tokio::test]
async fn test_world_contract_reader() {
    let sequencer = TestSequencer::start(SequencerConfig::default()).await;
    let account = sequencer.account();
    let provider = account.provider();
    let (world_address, executor_address) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap(),
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

    let strategy = prepare_for_migration(None, path, world).unwrap();
    let executor_address = strategy
        .executor
        .unwrap()
        .deploy(manifest.clone().executor.class_hash, vec![], &account)
        .await
        .unwrap()
        .contract_address;
    let world_address = strategy
        .world
        .unwrap()
        .deploy(manifest.clone().world.class_hash, vec![executor_address], &account)
        .await
        .unwrap()
        .contract_address;

    let mut declare_output = vec![];
    for component in strategy.components {
        let res = component.declare(&account).await.unwrap();
        declare_output.push(res);
    }

    let _ = WorldContract::new(world_address, &account)
        .register_components(&declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>())
        .await
        .unwrap();

    let mut declare_output = vec![];
    for system in strategy.systems {
        let res = system.declare(&account).await.unwrap();
        declare_output.push(res);
    }

    let world = WorldContract::new(world_address, &account);
    let _ = world
        .register_systems(&declare_output.iter().map(|o| o.class_hash).collect::<Vec<_>>())
        .await
        .unwrap();

    (world_address, executor_address)
}
