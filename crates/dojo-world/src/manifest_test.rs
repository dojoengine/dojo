use camino::Utf8PathBuf;
use dojo_test_utils::rpc::MockJsonRpcTransport;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, TestSequencer,
};
use serde_json::json;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{EmittedEvent, FieldElement};
use starknet::macros::{felt, short_string};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcMethod};

use super::{parse_contracts_events, Contract, Manifest, Model};
use crate::contracts::world::test::deploy_world;
use crate::manifest::{parse_models_events, ManifestError};
use crate::migration::world::WorldDiff;

#[tokio::test]
async fn manifest_from_remote_throw_error_on_not_deployed() {
    let mut mock_transport = MockJsonRpcTransport::new();
    mock_transport.set_response(
        JsonRpcMethod::GetClassHashAt,
        json!(["pending", "0x1"]),
        json!({
            "id": 1,
            "error": {
                "code": 20,
                "message": "Contract not found"
            },
        }),
    );

    let rpc = JsonRpcClient::new(mock_transport);
    let err = Manifest::load_from_remote(rpc, FieldElement::ONE).await.unwrap_err();

    match err {
        ManifestError::RemoteWorldNotFound => {
            // World not deployed.
        }
        err => panic!("Unexpected error: {err}"),
    }
}

#[test]
fn parse_registered_model_events() {
    let expected_models = vec![
        Model { name: "Model1".into(), class_hash: felt!("0x5555"), ..Default::default() },
        Model { name: "Model2".into(), class_hash: felt!("0x6666"), ..Default::default() },
    ];

    let events = vec![
        EmittedEvent {
            data: vec![short_string!("Model1"), felt!("0x5555"), felt!("0xbeef")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![short_string!("Model1"), felt!("0xbeef"), felt!("0")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![short_string!("Model2"), felt!("0x6666"), felt!("0")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
    ];

    let actual_models = parse_models_events(events);

    assert_eq!(actual_models.len(), 2);
    assert!(expected_models.contains(&actual_models[0]));
    assert!(expected_models.contains(&actual_models[1]));
}

#[test]
fn parse_deployed_contracts_events_without_upgrade() {
    let expected_contracts = vec![
        Contract {
            name: "".into(),
            class_hash: felt!("0x1"),
            address: Some(felt!("0x123")),
            ..Default::default()
        },
        Contract {
            name: "".into(),
            class_hash: felt!("0x2"),
            address: Some(felt!("0x456")),
            ..Default::default()
        },
        Contract {
            name: "".into(),
            class_hash: felt!("0x3"),
            address: Some(felt!("0x789")),
            ..Default::default()
        },
    ];

    let events = vec![
        EmittedEvent {
            data: vec![felt!("0x0"), felt!("0x1"), felt!("0x123")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x0"), felt!("0x2"), felt!("0x456")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x0"), felt!("0x3"), felt!("0x789")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
    ];

    let actual_contracts = parse_contracts_events(events, vec![]);
    assert_eq!(actual_contracts, expected_contracts);
}

#[test]
fn parse_deployed_contracts_events_with_upgrade() {
    let expected_contracts = vec![
        Contract {
            name: "".into(),
            class_hash: felt!("0x69"),
            address: Some(felt!("0x123")),
            ..Default::default()
        },
        Contract {
            name: "".into(),
            class_hash: felt!("0x2"),
            address: Some(felt!("0x456")),
            ..Default::default()
        },
        Contract {
            name: "".into(),
            class_hash: felt!("0x88"),
            address: Some(felt!("0x789")),
            ..Default::default()
        },
    ];

    let deployed_events = vec![
        EmittedEvent {
            data: vec![felt!("0x0"), felt!("0x1"), felt!("0x123")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x0"), felt!("0x2"), felt!("0x456")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x0"), felt!("0x3"), felt!("0x789")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
    ];

    let upgrade_events = vec![
        EmittedEvent {
            data: vec![felt!("0x66"), felt!("0x123")],
            keys: vec![],
            block_number: 2,
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x69"), felt!("0x123")],
            keys: vec![],
            block_number: 9,
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x77"), felt!("0x123")],
            keys: vec![],
            block_number: 5,
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x88"), felt!("0x789")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
    ];

    let actual_contracts = parse_contracts_events(deployed_events, upgrade_events);
    assert_eq!(actual_contracts, expected_contracts);
}

#[tokio::test(flavor = "multi_thread")]
async fn fetch_remote_manifest() {
    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let account = sequencer.account();
    let provider = account.provider();

    let artifacts_path =
        Utf8PathBuf::from_path_buf("../../examples/spawn-and-move/target/dev".into()).unwrap();
    let manifest_path = artifacts_path.join("manifest.json");

    let (world_address, _) = deploy_world(&sequencer, artifacts_path).await;

    let local_manifest = Manifest::load_from_path(manifest_path).unwrap();
    let remote_manifest = Manifest::load_from_remote(provider, world_address).await.unwrap();

    assert_eq!(local_manifest.models.len(), 2);
    assert_eq!(local_manifest.contracts.len(), 1);

    assert_eq!(remote_manifest.models.len(), 2);
    assert_eq!(remote_manifest.contracts.len(), 1);

    // compute diff from local and remote manifest

    let diff = WorldDiff::compute(local_manifest, Some(remote_manifest));

    assert_eq!(diff.count_diffs(), 0, "there should not be any diff");
}
