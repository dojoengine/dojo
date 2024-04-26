use std::io::Write;

use camino::Utf8PathBuf;
use dojo_lang::compiler::{BASE_DIR, MANIFESTS_DIR};
use dojo_test_utils::rpc::MockJsonRpcTransport;
use dojo_test_utils::compiler;
use katana_runner::KatanaRunner;
use serde_json::json;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::contract::AbiEntry;
use starknet::core::types::{EmittedEvent, FieldElement};
use starknet::macros::{felt, selector, short_string};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcMethod};

use super::{parse_contracts_events, AbiFormat, BaseManifest, DojoContract, DojoModel};
use crate::contracts::world::test::deploy_world;
use crate::manifest::{parse_models_events, AbstractManifestError, DeploymentManifest, Manifest};
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
    let err = DeploymentManifest::load_from_remote(rpc, FieldElement::ONE).await.unwrap_err();

    match err {
        AbstractManifestError::RemoteWorldNotFound => {
            // World not deployed.
        }
        err => panic!("Unexpected error: {err}"),
    }
}

#[test]
fn parse_registered_model_events() {
    let expected_models = vec![
        Manifest::new(
            DojoModel { class_hash: felt!("0x5555"), ..Default::default() },
            "Model1".into(),
        ),
        Manifest::new(
            DojoModel { class_hash: felt!("0x6666"), ..Default::default() },
            "Model2".into(),
        ),
    ];

    let selector = selector!("ModelRegistered");

    let events = vec![
        EmittedEvent {
            data: vec![
                short_string!("Model1"),
                felt!("0x5555"),
                felt!("0xbeef"),
                felt!("0xa1"),
                felt!("0"),
            ],
            keys: vec![selector],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![
                short_string!("Model1"),
                felt!("0xbeef"),
                felt!("0"),
                felt!("0xa1"),
                felt!("0xa1"),
            ],
            keys: vec![selector],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![
                short_string!("Model2"),
                felt!("0x6666"),
                felt!("0"),
                felt!("0xa3"),
                felt!("0"),
            ],
            keys: vec![selector],
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
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x1"),
                address: Some(felt!("0x123")),
                ..Default::default()
            },
            "".into(),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x2"),
                address: Some(felt!("0x456")),
                ..Default::default()
            },
            "".into(),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x3"),
                address: Some(felt!("0x789")),
                ..Default::default()
            },
            "".into(),
        ),
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
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x69"),
                address: Some(felt!("0x123")),
                ..Default::default()
            },
            "".into(),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x2"),
                address: Some(felt!("0x456")),
                ..Default::default()
            },
            "".into(),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x88"),
                address: Some(felt!("0x789")),
                ..Default::default()
            },
            "".into(),
        ),
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
            block_number: Some(2),
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x69"), felt!("0x123")],
            keys: vec![],
            block_number: Some(9),
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x77"), felt!("0x123")],
            keys: vec![],
            block_number: Some(5),
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x88"), felt!("0x789")],
            keys: vec![],
            block_number: Some(5),
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
    ];

    let actual_contracts = parse_contracts_events(deployed_events, upgrade_events);
    similar_asserts::assert_eq!(actual_contracts, expected_contracts);
}

#[test]
fn events_without_block_number_arent_parsed() {
    let expected_contracts = vec![
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x66"),
                address: Some(felt!("0x123")),
                ..Default::default()
            },
            "".into(),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x2"),
                address: Some(felt!("0x456")),
                ..Default::default()
            },
            "".into(),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x3"),
                address: Some(felt!("0x789")),
                ..Default::default()
            },
            "".into(),
        ),
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

    // only the first upgrade event has a block number and is parsed
    // meaning that the second and third upgrade events are ignored
    // and are not evaluated when parsing the remote contracts
    let upgrade_events = vec![
        EmittedEvent {
            data: vec![felt!("0x66"), felt!("0x123")],
            keys: vec![],
            block_number: Some(2),
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x69"), felt!("0x123")],
            keys: vec![],
            block_number: None,
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x77"), felt!("0x123")],
            keys: vec![],
            block_number: None,
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![felt!("0x88"), felt!("0x789")],
            keys: vec![],
            block_number: None,
            block_hash: Default::default(),
            from_address: Default::default(),
            transaction_hash: Default::default(),
        },
    ];

    let actual_contracts = parse_contracts_events(deployed_events, upgrade_events);
    similar_asserts::assert_eq!(actual_contracts, expected_contracts);
}

#[test]
fn fetch_remote_manifest() {
    let runner = KatanaRunner::new().expect("Fail to set runner");

    let account = runner.account(0);
    let provider = account.provider();

    let source_project = "../../examples/spawn-and-move/Scarb.toml";
    let dojo_core_path = "../dojo-core";
    let profile_name = "dev";

    // Build a completely new project in it's own directory.
    let (temp_project_dir, config, _) = compiler::copy_build_project_temp(source_project, dojo_core_path, true);

    let artifacts_path = temp_project_dir.join(format!("target/{profile_name}"));

    let world_address = config.tokio_handle().block_on(async {
        deploy_world(&runner, &temp_project_dir, &artifacts_path).await
    });

    let local_manifest = BaseManifest::load_from_path(
        &temp_project_dir.join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();

    let remote_manifest = config.tokio_handle().block_on(async {
        DeploymentManifest::load_from_remote(provider, world_address).await.unwrap()
    });

    assert_eq!(local_manifest.models.len(), 4);
    assert_eq!(local_manifest.contracts.len(), 1);

    assert_eq!(remote_manifest.models.len(), 4);
    assert_eq!(remote_manifest.contracts.len(), 1);

    // compute diff from local and remote manifest

    let diff = WorldDiff::compute(local_manifest, Some(remote_manifest));

    assert_eq!(diff.count_diffs(), 0, "there should not be any diff");
}

#[test]
fn test_abi_format_to_embed() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().join("abi.json");
    let mut temp_file = std::fs::File::create(&temp_path)?;

    let temp_dir_utf8 = Utf8PathBuf::from_path_buf(temp_dir.path().into()).unwrap();

    writeln!(
        temp_file,
        "[{{\"type\":\"function\",\"name\":\"testFunction\",\"inputs\":[],\"outputs\":[],\"\
         state_mutability\":\"view\"}}]"
    )?;

    let abi_format_path = AbiFormat::Path(Utf8PathBuf::from_path_buf(temp_path).unwrap());
    let embedded_abi = abi_format_path.to_embed(&temp_dir_utf8)?;

    let abi_format_not_changed = embedded_abi.clone();

    match &embedded_abi {
        AbiFormat::Embed(abi_entries) => {
            assert_eq!(abi_entries.len(), 1);
            let entry_0 = &abi_entries[0];
            if let AbiEntry::Function(function) = entry_0 {
                assert_eq!(function.name, "testFunction");
            }
        }
        _ => panic!("Expected AbiFormat::Embed variant"),
    }

    assert_eq!(embedded_abi, abi_format_not_changed.to_embed(&temp_dir_utf8).unwrap());

    Ok(())
}

#[test]
fn test_abi_format_to_path() {
    let embedded = AbiFormat::Embed(vec![]);
    assert!(embedded.to_path().is_none());

    let path = AbiFormat::Path(Utf8PathBuf::from("/tmp"));
    assert!(path.to_path().is_some());
}

#[test]
fn test_abi_format_load_abi_string() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().join("abi.json");
    let mut temp_file = std::fs::File::create(&temp_path)?;

    write!(temp_file, "[]")?;

    let path = AbiFormat::Path(Utf8PathBuf::from_path_buf(temp_path.clone()).unwrap());
    assert_eq!(path.load_abi_string(&Utf8PathBuf::new()).unwrap(), "[]");

    let embedded = AbiFormat::Embed(vec![]);
    assert_eq!(embedded.load_abi_string(&Utf8PathBuf::new()).unwrap(), "[]");

    Ok(())
}
