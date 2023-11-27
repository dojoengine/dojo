use camino::Utf8PathBuf;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use starknet::accounts::ConnectedAccount;
use starknet::core::types::FieldElement;

use crate::contracts::model::ModelReader;
use crate::contracts::world::test::deploy_world;
use crate::contracts::world::WorldContractReader;

#[tokio::test(flavor = "multi_thread")]
async fn test_model() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;
    let account = sequencer.account();
    let provider = account.provider();
    let (world_address, _) = deploy_world(
        &sequencer,
        Utf8PathBuf::from_path_buf("../../examples/spawn-and-move/target/dev".into()).unwrap(),
    )
    .await;

    let world = WorldContractReader::new(world_address, provider);
    let position = world.model("Position").await.unwrap();
    let schema = position.schema().await.unwrap();

    assert_eq!(
        schema,
        Ty::Struct(Struct {
            name: "Position".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    ty: Ty::Primitive(Primitive::ContractAddress(None)),
                    key: true
                },
                Member {
                    name: "vec".to_string(),
                    ty: Ty::Struct(Struct {
                        name: "Vec2".to_string(),
                        children: vec![
                            Member {
                                name: "x".to_string(),
                                ty: Ty::Primitive(Primitive::U32(None)),
                                key: false
                            },
                            Member {
                                name: "y".to_string(),
                                ty: Ty::Primitive(Primitive::U32(None)),
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
            "0x052a1da1853c194683ca5d6d154452d0654d23f2eacd4267c555ff2338e144d6"
        )
        .unwrap()
    );

    let moves = world.model("Moves").await.unwrap();
    let schema = moves.schema().await.unwrap();

    assert_eq!(
        schema,
        Ty::Struct(Struct {
            name: "Moves".to_string(),
            children: vec![
                Member {
                    name: "player".to_string(),
                    ty: Ty::Primitive(Primitive::ContractAddress(None)),
                    key: true
                },
                Member {
                    name: "remaining".to_string(),
                    ty: Ty::Primitive(Primitive::U8(None)),
                    key: false
                },
                Member {
                    name: "last_direction".to_string(),
                    ty: Ty::Enum(Enum {
                        name: "Direction".to_string(),
                        option: None,
                        options: vec![
                            EnumOption { name: "None".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "Left".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "Right".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "Up".to_string(), ty: Ty::Tuple(vec![]) },
                            EnumOption { name: "Down".to_string(), ty: Ty::Tuple(vec![]) },
                        ]
                    }),
                    key: false
                }
            ]
        })
    );
}
