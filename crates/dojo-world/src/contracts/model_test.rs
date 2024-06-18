use camino::Utf8PathBuf;
use dojo_test_utils::compiler;
use dojo_types::primitive::Primitive;
use dojo_types::schema::{Enum, EnumOption, Member, Struct, Ty};
use katana_runner::KatanaRunner;
use starknet::accounts::ConnectedAccount;
use starknet::macros::felt;

use crate::contracts::model::ModelReader;
use crate::contracts::world::test::deploy_world;
use crate::contracts::world::WorldContractReader;
use crate::metadata::dojo_metadata_from_workspace;

#[tokio::test(flavor = "multi_thread")]
async fn test_model() {
    let runner = KatanaRunner::new().expect("Fail to set runner");
    let account = runner.account(0);
    let provider = account.provider();

    let config = compiler::copy_tmp_config(
        &Utf8PathBuf::from("../../examples/spawn-and-move"),
        &Utf8PathBuf::from("../dojo-core"),
    );

    let manifest_dir = config.manifest_path().parent().unwrap();
    let target_dir = manifest_dir.join("target").join("dev");

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    let world_address =
        deploy_world(&runner, &manifest_dir.into(), &target_dir, dojo_metadata.skip_migration)
            .await;

    let world = WorldContractReader::new(world_address, provider);
    let position = world.model_reader("Position").await.unwrap();
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
        felt!("0x027942375b09862291ece780c573e8c625df4ba41fd7524e0658ca75fff014ff")
    );

    let moves = world.model_reader("Moves").await.unwrap();
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
