use camino::Utf8PathBuf;
use dojo_test_utils::migration::{copy_spawn_and_move_db, prepare_migration_with_world_and_seed};
use dojo_test_utils::setup::TestSetup;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use katana_runner::RunnerCtx;
use scarb_interop::Profile;
use starknet::accounts::ConnectedAccount;
use starknet::macros::felt;

use crate::contracts::model::ModelReader;
use crate::contracts::world::WorldContractReader;

#[tokio::test(flavor = "multi_thread")]
#[katana_runner::test(db_dir = copy_spawn_and_move_db().as_str())]
async fn test_model(sequencer: &RunnerCtx) {
    let account = sequencer.account(0);
    let provider = account.provider();

    let setup = TestSetup::from_examples("../dojo/core", "../../examples/");

    let manifest_dir = setup.manifest_dir("spawn-and-move");
    let target_dir = manifest_dir.join("target").join("dev");

    let (strat, _) = prepare_migration_with_world_and_seed(
        Utf8PathBuf::from(&manifest_dir),
        Utf8PathBuf::from(&target_dir),
        None,
        "dojo_examples",
        "dojo_examples",
    )
    .unwrap();

    let world = WorldContractReader::new(strat.world_address, provider);
    let position = world.model_reader("dojo_examples", "Position").await.unwrap();
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
        felt!("0x5af60d63e6a1d25fc117fde1fa7e1d628adc46a52c3d007541ed6dd369e8ea")
    );

    // accessing to an unknown model should return an error
    let res = world.model_reader("dojo_examples", "UnknownModel").await;
    assert!(res.is_err());

    let moves = world.model_reader("dojo_examples", "Moves").await.unwrap();
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
