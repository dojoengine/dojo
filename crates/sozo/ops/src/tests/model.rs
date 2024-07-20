use dojo_world::contracts::abi::model::{FieldLayout, Layout};
use dojo_world::contracts::world::WorldContract;
use dojo_world::migration::TxnConfig;
use katana_runner::KatanaRunner;
use scarb_ui::{OutputFormat, Ui, Verbosity};
use starknet::accounts::Account;
use starknet::core::types::Felt;

use super::setup;
use crate::{execute, model};

// Test model ops in the same to avoid spinning up several katana with full
// migration for now. Should be replaced by individual tests once Katana spinning up is enhanced.
#[tokio::test(flavor = "multi_thread")]
async fn test_model_ops() {
    let sequencer = KatanaRunner::new().expect("Failed to start runner.");

    let world = setup::setup(&sequencer).await.unwrap();

    assert_eq!(
        model::model_class_hash(
            "dojo_examples-Moves".to_string(),
            world.address,
            sequencer.provider()
        )
        .await
        .unwrap(),
        Felt::from_hex("0x47f77d27573e12c1441dd65b1a88f4432c1c4ec446241830ff4c33f3f58a44e")
            .unwrap()
    );

    assert_eq!(
        model::model_contract_address(
            "dojo_examples-Moves".to_string(),
            world.address,
            sequencer.provider()
        )
        .await
        .unwrap(),
        Felt::from_hex("0x413aab3045f0a59103449d05974e30aaf91985113b0ae5d97a43b1141c3e1cb")
            .unwrap()
    );

    let layout =
        model::model_layout("dojo_examples-Moves".to_string(), world.address, sequencer.provider())
            .await
            .unwrap();

    let expected_layout = Layout::Struct(vec![
        FieldLayout {
            selector: Felt::from_hex(
                "0x2d09b71759c924026f2006fa173772a54e6cd329e2f4083e6b5742463843116",
            )
            .unwrap(),
            layout: Layout::Fixed(vec![8]),
        },
        FieldLayout {
            selector: Felt::from_hex(
                "0x38717e79a678d35c1e9a8af2ea98a46dbfd566b6dd257bb4cdabea227c469a2",
            )
            .unwrap(),
            layout: Layout::Enum(vec![
                FieldLayout { selector: Felt::from(0x0), layout: Layout::Fixed(vec![]) },
                FieldLayout { selector: Felt::from(0x1), layout: Layout::Fixed(vec![]) },
                FieldLayout { selector: Felt::from(0x2), layout: Layout::Fixed(vec![]) },
                FieldLayout { selector: Felt::from(0x3), layout: Layout::Fixed(vec![]) },
                FieldLayout { selector: Felt::from(0x4), layout: Layout::Fixed(vec![]) },
            ]),
        },
    ]);

    assert_eq!(layout, expected_layout);

    let schema = model::model_schema(
        "dojo_examples-Moves".to_string(),
        world.address,
        sequencer.provider(),
        true,
    )
    .await
    .unwrap();

    let expected_schema = dojo_types::schema::Ty::Struct(dojo_types::schema::Struct {
        name: "Moves".to_string(),
        children: vec![
            dojo_types::schema::Member {
                name: "player".to_string(),
                ty: dojo_types::schema::Ty::Primitive(
                    dojo_types::primitive::Primitive::ContractAddress(None),
                ),
                key: true,
            },
            dojo_types::schema::Member {
                name: "remaining".to_string(),
                ty: dojo_types::schema::Ty::Primitive(dojo_types::primitive::Primitive::U8(None)),
                key: false,
            },
            dojo_types::schema::Member {
                name: "last_direction".to_string(),
                ty: dojo_types::schema::Ty::Enum(dojo_types::schema::Enum {
                    name: "Direction".to_string(),
                    option: None,
                    options: vec![
                        dojo_types::schema::EnumOption {
                            name: "None".to_string(),
                            ty: dojo_types::schema::Ty::Tuple(vec![]),
                        },
                        dojo_types::schema::EnumOption {
                            name: "Left".to_string(),
                            ty: dojo_types::schema::Ty::Tuple(vec![]),
                        },
                        dojo_types::schema::EnumOption {
                            name: "Right".to_string(),
                            ty: dojo_types::schema::Ty::Tuple(vec![]),
                        },
                        dojo_types::schema::EnumOption {
                            name: "Up".to_string(),
                            ty: dojo_types::schema::Ty::Tuple(vec![]),
                        },
                        dojo_types::schema::EnumOption {
                            name: "Down".to_string(),
                            ty: dojo_types::schema::Ty::Tuple(vec![]),
                        },
                    ],
                }),
                key: false,
            },
        ],
    });

    assert_eq!(schema, expected_schema);

    let expected_values = vec![Felt::from(0x0), Felt::from(0x0)];

    let (_, values) = model::model_get(
        "dojo_examples-Moves".to_string(),
        vec![sequencer.account(0).address()],
        world.address,
        sequencer.provider(),
    )
    .await
    .unwrap();

    assert_eq!(values, expected_values);

    let _r = execute::execute(
        &Ui::new(Verbosity::Normal, OutputFormat::Text),
        "dojo_examples-actions".to_string(),
        "spawn".to_string(),
        vec![],
        &WorldContract::new(world.address, sequencer.account(0)),
        &TxnConfig::init_wait(),
    )
    .await;

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let expected_values = vec![Felt::from(99), Felt::from(0x0)];

    let (_, values) = model::model_get(
        "dojo_examples-Moves".to_string(),
        vec![sequencer.account(0).address()],
        world.address,
        sequencer.provider(),
    )
    .await
    .unwrap();

    assert_eq!(values, expected_values);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_check_tag_or_read_default() {
    let config = setup::load_config();

    let tag = model::check_tag_or_read_default_namespace("Moves", &config).unwrap();
    assert_eq!(tag, "dojo_examples-Moves");

    let tag = model::check_tag_or_read_default_namespace("dojo_examples-Moves", &config).unwrap();
    assert_eq!(tag, "dojo_examples-Moves");
}
