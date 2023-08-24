use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use starknet::accounts::Account;
use starknet::core::types::{BlockId, BlockTag};
use starknet_crypto::FieldElement;

use crate::contract::world::test::deploy_world;
use crate::contract::world::WorldContract;

#[tokio::test]
async fn test_system() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();
    let (world_address, _) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../../examples/ecs/target/dev".into()).unwrap(),
    )
    .await;

    let block_id: BlockId = BlockId::Tag(BlockTag::Latest);
    let world = WorldContract::new(world_address, &account);
    let spawn = world.system("spawn", block_id).await.unwrap();

    let _ = spawn.execute(vec![]).await.unwrap();

    let component = world.component("Moves", block_id).await.unwrap();
    let moves = component.entity(vec![account.address()], block_id).await.unwrap();

    assert_eq!(moves, vec![10_u8.into()]);

    let move_system = world.system("move", block_id).await.unwrap();

    let _ = move_system.execute(vec![FieldElement::ONE]).await.unwrap();
    let _ = move_system.execute(vec![FieldElement::THREE]).await.unwrap();

    let moves = component.entity(vec![account.address()], block_id).await.unwrap();

    assert_eq!(moves, vec![8_u8.into()]);

    let position_component = world.component("Position", block_id).await.unwrap();

    let position = position_component.entity(vec![account.address()], block_id).await.unwrap();

    assert_eq!(position, vec![11_u8.into(), 11_u8.into()]);
}
