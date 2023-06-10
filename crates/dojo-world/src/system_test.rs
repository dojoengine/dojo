use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::TestSequencer;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag};

use crate::manifest::Dependency;
use crate::world::test::deploy_world;
use crate::world::WorldContractReader;

#[tokio::test]
async fn test_system() {
    let sequencer = TestSequencer::start().await;
    let account = sequencer.account();
    let provider = account.provider();
    let (world_address, _) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap(),
    )
    .await;

    let block_id: BlockId = BlockId::Tag(BlockTag::Latest);
    let world = WorldContractReader::new(world_address, provider);
    let system = world.system("Spawn", block_id).await.unwrap();
    let dependencies = system.dependencies(block_id).await.unwrap();
    assert_eq!(
        dependencies,
        vec![
            Dependency { name: "Moves".into(), read: false, write: true },
            Dependency { name: "Position".into(), read: false, write: true }
        ]
    );
}
