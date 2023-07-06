use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{SequencerConfig, TestSequencer};
use dojo_types::component::Member;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, FieldElement};

use crate::contract::world::test::deploy_world;
use crate::contract::world::WorldContractReader;

#[tokio::test]
async fn test_component() {
    let sequencer = TestSequencer::start(SequencerConfig::default()).await;
    let account = sequencer.account();
    let provider = account.provider();
    let (world_address, _) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev".into()).unwrap(),
    )
    .await;

    let block_id = BlockId::Tag(BlockTag::Latest);
    let world = WorldContractReader::new(world_address, provider);
    let component = world.component("Position", block_id).await.unwrap();

    assert_eq!(
        component.class_hash(),
        FieldElement::from_hex_be(
            "0x0669ea11a6f52d1330f76322e0c2393fb72a73e0cbc05359854f2a539743d000"
        )
        .unwrap()
    );

    let members = component.schema(block_id).await.unwrap();

    assert_eq!(
        members,
        vec![
            Member { name: "x".into(), ty: "u32".into(), slot: 0, offset: 0 },
            Member { name: "y".into(), ty: "u32".into(), slot: 1, offset: 0 }
        ]
    )
}
