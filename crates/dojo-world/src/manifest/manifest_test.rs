use std::io::Write;

use cainome::cairo_serde::{ByteArray, CairoSerde};
use camino::Utf8PathBuf;
use dojo_test_utils::compiler::CompilerTestSetup;
use dojo_test_utils::migration::{copy_spawn_and_move_db, prepare_migration_with_world_and_seed};
use dojo_test_utils::rpc::MockJsonRpcTransport;
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use scarb::compiler::Profile;
use serde_json::json;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::contract::AbiEntry;
use starknet::core::types::{EmittedEvent, Felt};
use starknet::macros::{felt, selector};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcMethod};

use super::{
    parse_contracts_events, AbiFormat, BaseManifest, DojoContract, DojoModel, OverlayDojoContract,
    OverlayManifest,
};
use crate::contracts::naming::{get_filename_from_tag, get_tag};
use crate::manifest::{
    parse_models_events, AbstractManifestError, DeploymentManifest, Manifest, OverlayClass,
    OverlayDojoModel, BASE_DIR, MANIFESTS_DIR, OVERLAYS_DIR,
};
use crate::metadata::dojo_metadata_from_workspace;
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
    let err = DeploymentManifest::load_from_remote(rpc, Felt::ONE).await.unwrap_err();

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
            DojoModel {
                tag: get_tag("ns", "modelA"),
                class_hash: felt!("0x5555"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns", "modelA")),
        ),
        Manifest::new(
            DojoModel {
                tag: get_tag("ns", "modelB"),
                class_hash: felt!("0x6666"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns", "modelB")),
        ),
    ];

    let events = vec![
        build_model_registered_event(vec![felt!("0x5555"), felt!("0xbeef")], "ns", "modelA"),
        build_model_registered_event(vec![felt!("0x5555"), felt!("0xbeef")], "ns", "modelA"),
        build_model_registered_event(vec![felt!("0x6666"), felt!("0xbaaf")], "ns", "modelB"),
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
                tag: get_tag("ns1", "c1"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns1", "c1")),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x2"),
                address: Some(felt!("0x456")),
                tag: get_tag("ns2", "c2"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns2", "c2")),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x3"),
                address: Some(felt!("0x789")),
                tag: get_tag("ns3", "c3"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns3", "c3")),
        ),
    ];

    let events = vec![
        build_deploy_event(vec![felt!("0x0"), felt!("0x1"), felt!("0x123")], "ns1", "c1"),
        build_deploy_event(vec![felt!("0x0"), felt!("0x2"), felt!("0x456")], "ns2", "c2"),
        build_deploy_event(vec![felt!("0x0"), felt!("0x3"), felt!("0x789")], "ns3", "c3"),
    ];

    let actual_contracts = parse_contracts_events(events, vec![], vec![]);
    assert_eq!(actual_contracts, expected_contracts);
}

#[test]
fn parse_deployed_contracts_events_with_upgrade() {
    let expected_contracts = vec![
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x69"),
                address: Some(felt!("0x123")),
                tag: get_tag("ns1", "c1"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns1", "c1")),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x2"),
                address: Some(felt!("0x456")),
                tag: get_tag("ns2", "c2"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns2", "c2")),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x88"),
                address: Some(felt!("0x789")),
                tag: get_tag("ns3", "c3"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns3", "c3")),
        ),
    ];

    let deployed_events = vec![
        build_deploy_event(vec![felt!("0x0"), felt!("0x1"), felt!("0x123")], "ns1", "c1"),
        build_deploy_event(vec![felt!("0x0"), felt!("0x2"), felt!("0x456")], "ns2", "c2"),
        build_deploy_event(vec![felt!("0x0"), felt!("0x3"), felt!("0x789")], "ns3", "c3"),
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

    let actual_contracts = parse_contracts_events(deployed_events, upgrade_events, vec![]);
    similar_asserts::assert_eq!(actual_contracts, expected_contracts);
}

#[test]
fn events_without_block_number_arent_parsed() {
    let expected_contracts = vec![
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x66"),
                address: Some(felt!("0x123")),
                tag: get_tag("ns1", "c1"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns1", "c1")),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x2"),
                address: Some(felt!("0x456")),
                tag: get_tag("ns2", "c2"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns2", "c2")),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x3"),
                address: Some(felt!("0x789")),
                tag: get_tag("ns3", "c3"),
                ..Default::default()
            },
            get_filename_from_tag(&get_tag("ns3", "c3")),
        ),
    ];

    let deployed_events = vec![
        build_deploy_event(vec![felt!("0x0"), felt!("0x1"), felt!("0x123")], "ns1", "c1"),
        build_deploy_event(vec![felt!("0x0"), felt!("0x2"), felt!("0x456")], "ns2", "c2"),
        build_deploy_event(vec![felt!("0x0"), felt!("0x3"), felt!("0x789")], "ns3", "c3"),
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

    let actual_contracts = parse_contracts_events(deployed_events, upgrade_events, vec![]);
    similar_asserts::assert_eq!(actual_contracts, expected_contracts);
}

#[test]
fn fetch_remote_manifest() {
    let seq_config = KatanaRunnerConfig::default().with_db_dir(copy_spawn_and_move_db().as_str());
    let sequencer = KatanaRunner::new_with_config(seq_config).expect("Failed to start runner.");

    let account = sequencer.account(0);
    let provider = account.provider();

    let setup = CompilerTestSetup::from_examples("../dojo-core", "../../examples/");
    let config = setup.build_test_config("spawn-and-move", Profile::DEV);
    let profile_name = Profile::DEV.to_string();

    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
    let manifest_path = Utf8PathBuf::from(config.manifest_path().parent().unwrap());
    let target_dir = Utf8PathBuf::from(ws.target_dir().to_string()).join("dev");
    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    let (strat, _) = prepare_migration_with_world_and_seed(
        manifest_path.clone(),
        target_dir,
        None,
        "dojo_examples",
        "dojo_examples",
    )
    .unwrap();

    let mut local_manifest = BaseManifest::load_from_path(
        &manifest_path.join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    if let Some(m) = dojo_metadata.migration {
        local_manifest.remove_tags(&m.skip_contracts);
    }

    let overlay_dir = manifest_path.join(OVERLAYS_DIR).join(&profile_name);
    if overlay_dir.exists() {
        let overlay_manifest =
            OverlayManifest::load_from_path(&overlay_dir, &local_manifest).unwrap();

        local_manifest.merge(overlay_manifest);
    }

    let remote_manifest = config.tokio_handle().block_on(async {
        DeploymentManifest::load_from_remote(provider, strat.world_address).await.unwrap()
    });

    assert_eq!(local_manifest.models.len(), 10);
    assert_eq!(local_manifest.contracts.len(), 4);

    assert_eq!(remote_manifest.models.len(), 10);
    assert_eq!(remote_manifest.contracts.len(), 4);

    // compute diff from local and remote manifest

    let diff = WorldDiff::compute(local_manifest, Some(remote_manifest), "dojo-test").unwrap();

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

#[test]
fn overlay_merge_for_contract_and_model_work_as_expected() {
    let other = OverlayManifest {
        contracts: vec![
            OverlayDojoContract { tag: "ns:othercontract1".into(), ..Default::default() },
            OverlayDojoContract { tag: "ns:othercontract2".into(), ..Default::default() },
            OverlayDojoContract { tag: "ns:existingcontract".into(), ..Default::default() },
        ],
        models: vec![
            OverlayDojoModel { tag: "ns:othermodel1".into(), ..Default::default() },
            OverlayDojoModel { tag: "ns:othermodel2".into(), ..Default::default() },
            OverlayDojoModel { tag: "ns:existingmodel".into(), ..Default::default() },
        ],
        ..Default::default()
    };

    let mut current = OverlayManifest {
        contracts: vec![
            OverlayDojoContract { tag: "ns:currentcontract1".into(), ..Default::default() },
            OverlayDojoContract { tag: "ns:currentcontract2".into(), ..Default::default() },
            OverlayDojoContract { tag: "ns:existingcontract".into(), ..Default::default() },
        ],
        models: vec![
            OverlayDojoModel { tag: "ns:currentmodel1".into(), ..Default::default() },
            OverlayDojoModel { tag: "ns:currentmodel2".into(), ..Default::default() },
            OverlayDojoModel { tag: "ns:existingmodel".into(), ..Default::default() },
        ],
        ..Default::default()
    };

    let expected = OverlayManifest {
        contracts: vec![
            OverlayDojoContract { tag: "ns:currentcontract1".into(), ..Default::default() },
            OverlayDojoContract { tag: "ns:currentcontract2".into(), ..Default::default() },
            OverlayDojoContract { tag: "ns:existingcontract".into(), ..Default::default() },
            OverlayDojoContract { tag: "ns:othercontract1".into(), ..Default::default() },
            OverlayDojoContract { tag: "ns:othercontract2".into(), ..Default::default() },
        ],
        models: vec![
            OverlayDojoModel { tag: "ns:currentmodel1".into(), ..Default::default() },
            OverlayDojoModel { tag: "ns:currentmodel2".into(), ..Default::default() },
            OverlayDojoModel { tag: "ns:existingmodel".into(), ..Default::default() },
            OverlayDojoModel { tag: "ns:othermodel1".into(), ..Default::default() },
            OverlayDojoModel { tag: "ns:othermodel2".into(), ..Default::default() },
        ],
        ..Default::default()
    };

    current.merge(other);

    assert_eq!(current, expected);
}

#[test]
fn overlay_merge_for_world_work_as_expected() {
    // when other.world is none and current.world is some
    let other = OverlayManifest { ..Default::default() };
    let mut current = OverlayManifest {
        world: Some(OverlayClass { tag: "dojo:world".to_string(), ..Default::default() }),
        ..Default::default()
    };
    let expected = OverlayManifest {
        world: Some(OverlayClass { tag: "dojo:world".to_string(), ..Default::default() }),
        ..Default::default()
    };
    current.merge(other);

    assert_eq!(current, expected);

    // when other.world is some and current.world is none
    let other = OverlayManifest {
        world: Some(OverlayClass { tag: "dojo:world".to_string(), ..Default::default() }),
        ..Default::default()
    };
    let mut current = OverlayManifest { ..Default::default() };
    let expected = OverlayManifest {
        world: Some(OverlayClass { tag: "dojo:world".to_string(), ..Default::default() }),
        ..Default::default()
    };

    current.merge(other);
    assert_eq!(current, expected);

    // when other.world is some and current.world is some
    let other = OverlayManifest {
        world: Some(OverlayClass { tag: "dojo:worldother".to_string(), ..Default::default() }),
        ..Default::default()
    };
    let mut current = OverlayManifest {
        world: Some(OverlayClass { tag: "dojo:worldcurrent".to_string(), ..Default::default() }),
        ..Default::default()
    };
    let expected = OverlayManifest {
        world: Some(OverlayClass { tag: "dojo:worldcurrent".to_string(), ..Default::default() }),
        ..Default::default()
    };

    current.merge(other);
    assert_eq!(current, expected);

    // when other.world is none and current.world is none
    let other = OverlayManifest { ..Default::default() };
    let mut current = OverlayManifest { ..Default::default() };
    let expected = OverlayManifest { ..Default::default() };

    current.merge(other);
    assert_eq!(current, expected);
}

#[test]
fn overlay_merge_for_base_work_as_expected() {
    // when other.base is none and current.base is some
    let other = OverlayManifest { ..Default::default() };
    let mut current = OverlayManifest {
        base: Some(OverlayClass { tag: "dojo:base".to_string(), ..Default::default() }),
        ..Default::default()
    };
    let expected = OverlayManifest {
        base: Some(OverlayClass { tag: "dojo:base".to_string(), ..Default::default() }),
        ..Default::default()
    };
    current.merge(other);

    assert_eq!(current, expected);

    // when other.base is some and current.base is none
    let other = OverlayManifest {
        base: Some(OverlayClass { tag: "dojo:base".to_string(), ..Default::default() }),
        ..Default::default()
    };
    let mut current = OverlayManifest { ..Default::default() };
    let expected = OverlayManifest {
        base: Some(OverlayClass { tag: "dojo:base".to_string(), ..Default::default() }),
        ..Default::default()
    };

    current.merge(other);
    assert_eq!(current, expected);

    // when other.base is some and current.base is some
    let other = OverlayManifest {
        base: Some(OverlayClass { tag: "dojo:baseother".to_string(), ..Default::default() }),
        ..Default::default()
    };
    let mut current = OverlayManifest {
        base: Some(OverlayClass { tag: "dojo:basecurrent".to_string(), ..Default::default() }),
        ..Default::default()
    };
    let expected = OverlayManifest {
        base: Some(OverlayClass { tag: "dojo:basecurrent".to_string(), ..Default::default() }),
        ..Default::default()
    };

    current.merge(other);
    assert_eq!(current, expected);

    // when other.base is none and current.base is none
    let other = OverlayManifest { ..Default::default() };
    let mut current = OverlayManifest { ..Default::default() };
    let expected = OverlayManifest { ..Default::default() };

    current.merge(other);
    assert_eq!(current, expected);
}

#[test]
fn base_manifest_remove_items_work_as_expected() {
    let contracts = ["ns:c1", "ns:c2", "ns:c3"];
    let models = ["ns:m1", "ns:m2", "ns:m3"];

    let world = Manifest { manifest_name: "world".into(), inner: Default::default() };
    let base = Manifest { manifest_name: "dojo-base".to_string(), inner: Default::default() };

    let contracts = contracts
        .iter()
        .map(|c| Manifest {
            manifest_name: c.to_string(),
            inner: DojoContract { tag: c.to_string(), ..Default::default() },
        })
        .collect();
    let models = models
        .iter()
        .map(|c| Manifest {
            manifest_name: c.to_string(),
            inner: DojoModel { tag: c.to_string(), ..Default::default() },
        })
        .collect();

    let mut base = BaseManifest { contracts, models, world, base };

    base.remove_tags(&["ns:c1".to_string(), "ns:c3".to_string(), "ns:m2".to_string()]);

    assert_eq!(base.contracts.len(), 1);
    assert_eq!(
        base.contracts.iter().map(|c| c.manifest_name.clone()).collect::<Vec<String>>(),
        vec!["ns:c2"]
    );

    assert_eq!(base.models.len(), 2);
    assert_eq!(
        base.models.iter().map(|c| c.manifest_name.clone()).collect::<Vec<String>>(),
        vec!["ns:m1", "ns:m3"]
    );
}

fn serialize_bytearray(s: &str) -> Vec<Felt> {
    let ba = ByteArray::from_string(s).unwrap();
    ByteArray::cairo_serialize(&ba)
}

fn build_model_registered_event(values: Vec<Felt>, namespace: &str, model: &str) -> EmittedEvent {
    let mut data = ByteArray::cairo_serialize(&ByteArray::from_string(model).unwrap());
    data.extend(ByteArray::cairo_serialize(&ByteArray::from_string(namespace).unwrap()));
    data.extend(values);

    EmittedEvent {
        data,
        keys: vec![selector!("ModelRegistered")],
        block_hash: Default::default(),
        from_address: Default::default(),
        block_number: Default::default(),
        transaction_hash: Default::default(),
    }
}

fn build_deploy_event(values: Vec<Felt>, ns: &str, name: &str) -> EmittedEvent {
    let mut data = values.to_vec();
    data.extend(serialize_bytearray(ns).iter());
    data.extend(serialize_bytearray(name).iter());

    EmittedEvent {
        data,
        keys: vec![],
        block_hash: Default::default(),
        from_address: Default::default(),
        block_number: Default::default(),
        transaction_hash: Default::default(),
    }
}
