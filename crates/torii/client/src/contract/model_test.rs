use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_types::model::{Enum, Member, Struct, Ty};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, FieldElement};

use crate::contract::world::test::deploy_world;
use crate::contract::world::WorldContractReader;

#[tokio::test(flavor = "multi_thread")]
async fn test_model() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();
    let provider = account.provider();
    let (world_address, _) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../../examples/ecs/target/dev".into()).unwrap(),
    )
    .await;

    let block_id = BlockId::Tag(BlockTag::Latest);
    let world = WorldContractReader::new(world_address, provider);
    let position = world.model("Position", block_id).await.unwrap();
    let schema = position.schema(block_id).await.unwrap();

    assert_eq!(
        schema,
        Ty::Struct(Struct {
            name: "Position".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    ty: Ty::Terminal("ContractAddress".to_string()),
                    key: true
                },
                Member {
                    name: "vec".to_string(),
                    ty: Ty::Struct(Struct {
                        name: "Vec2".to_string(),
                        children: vec![
                            Member {
                                name: "x".to_string(),
                                ty: Ty::Terminal("u32".to_string()),
                                key: false
                            },
                            Member {
                                name: "y".to_string(),
                                ty: Ty::Terminal("u32".to_string()),
                                key: false
                            }
                        ]
                    }),
                    key: false
                }
            ]
        })
    );

    assert_eq!(
        position.class_hash(),
        FieldElement::from_hex_be(
            "0x07a812f2cfb414d5aa04bb9a3c91cdcaf1d30e193bd6cb7faf9b7c294722fab4"
        )
        .unwrap()
    );

    let moves = world.model("Moves", block_id).await.unwrap();
    let schema = moves.schema(block_id).await.unwrap();

    assert_eq!(
        schema,
        Ty::Struct(Struct {
            name: "Moves".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    ty: Ty::Terminal("ContractAddress".to_string()),
                    key: true
                },
                Member {
                    name: "remaining".to_string(),
                    ty: Ty::Terminal("u8".to_string()),
                    key: false
                },
                Member {
                    name: "last_direction".to_string(),
                    ty: Ty::Enum(Enum {
                        name: "Direction".to_string(),
                        children: vec![
                            Ty::Terminal("None".to_string()),
                            Ty::Terminal("Left".to_string()),
                            Ty::Terminal("Right".to_string()),
                            Ty::Terminal("Up".to_string()),
                            Ty::Terminal("Down".to_string())
                        ]
                    }),
                    key: false
                }
            ]
        })
    );
}
