use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{SequencerConfig, TestSequencer};
use dojo_types::system::Dependency;
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};
use starknet_crypto::FieldElement;

use crate::contract::world::test::deploy_world;
use crate::contract::world::WorldContract;

#[tokio::test]
async fn test_system() {
    let sequencer = TestSequencer::start(SequencerConfig::default()).await;
    let account = sequencer.account();
    let (world_address, _) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap(),
    )
    .await;

    let block_id: BlockId = BlockId::Tag(BlockTag::Latest);
    let world = WorldContract::new(world_address, &account);
    let spawn = world.system("spawn", block_id).await.unwrap();
    let dependencies = spawn.dependencies(block_id).await.unwrap();
    assert_eq!(
        dependencies,
        vec![
            Dependency { name: "Moves".into(), read: false, write: true },
            Dependency { name: "Position".into(), read: false, write: true }
        ]
    );

    let _ = spawn.execute(vec![]).await.unwrap();

    let component = world.component("Moves", block_id).await.unwrap();
    let moves =
        component.entity(FieldElement::ZERO, vec![account.address()], block_id).await.unwrap();

    assert_eq!(moves, vec![10_u8.into()]);

    let move_system = world.system("move", block_id).await.unwrap();

    let _ = move_system.execute(vec![FieldElement::ONE]).await.unwrap();
    let _ = move_system.execute(vec![FieldElement::THREE]).await.unwrap();

    let moves =
        component.entity(FieldElement::ZERO, vec![account.address()], block_id).await.unwrap();

    assert_eq!(moves, vec![8_u8.into()]);

    let position_component = world.component("Position", block_id).await.unwrap();

    let position = position_component
        .entity(FieldElement::ZERO, vec![account.address()], block_id)
        .await
        .unwrap();

    assert_eq!(position, vec![1_u8.into(), 1_u8.into()]);
}
