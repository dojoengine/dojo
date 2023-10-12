use std::collections::HashMap;

use dojo_test_utils::rpc::MockJsonRpcTransport;
use serde_json::json;
use starknet::core::types::{EmittedEvent, FieldElement};
use starknet::macros::{felt, short_string};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcMethod};

use super::{
    parse_deployed_contracts_events, parse_registered_model_events, Class, Contract, Manifest,
    Model, BASE_CONTRACT_NAME, EXECUTOR_CONTRACT_NAME, WORLD_CONTRACT_NAME,
};
use crate::manifest::ManifestError;

fn create_example_remote_manifest() -> Manifest {
    Manifest {
        world: Contract {
            abi: None,
            name: WORLD_CONTRACT_NAME.into(),
            address: Some(felt!(
                "0x04e10dcec3ed05fecb289ec2aefd81a0e73ba0dfb66c8e6b01e20593f471c70c"
            )),
            class_hash: felt!("0x05c3494b21bc92d40abdc40cdc54af66f22fb92bf876665d982c765a2cc0e06a"),
        },
        executor: Contract {
            abi: None,
            address: Some(felt!(
                "0x05c3494b21bc92d40abdc40cdc54af66f22fb92bf876665d982c765a2cc0e06a"
            )),
            class_hash: felt!("0x02b35dd4816731188ed1ad16caa73bde76075c9d9cb8cbfa3e447d3ab9b1ab33"),
            name: EXECUTOR_CONTRACT_NAME.into(),
        },
        base: Class {
            name: BASE_CONTRACT_NAME.into(),
            class_hash: felt!("0x07aec2b7d7064c1294a339cd90060331ff704ab573e4ee9a1b699be2215c11c9"),
            abi: None,
        },
        contracts: vec![Contract {
            name: "player_actions".into(),
            address: Some(felt!(
                "0x06f359762e26f9d5562284ec8c55c7d4854a8a90fdcdc09795e31a8d78fc6221"
            )),
            class_hash: felt!("0x0723e7c7e8748c0a81bf4b426e3c5dee84df728d1630324d75077a21b0271bb4"),
            abi: None,
        }],
        models: vec![
            Model {
                name: "Position".into(),
                class_hash: felt!(
                    "0x06ffc643cbc4b2fb9c424242b18175a5e142269b45f4463d1cd4dddb7a2e5095"
                ),
                ..Default::default()
            },
            Model {
                name: "Moves".into(),
                class_hash: felt!(
                    "0x07a3234437ebfadbae8465c1e81660c714e09bb77fa248ccfe66c6f3e6a03698"
                ),
                ..Default::default()
            },
        ],
    }
}

#[tokio::test]
async fn test_manifest_from_remote_throw_error_on_not_deployed() {
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
fn test_parse_registered_model_events() {
    let expected_models = create_example_remote_manifest().models;

    let events = vec![
        EmittedEvent {
            data: vec![
                short_string!("Position"),
                felt!("0x06ffc643cbc4b2fb9c424242b18175a5e142269b45f4463d1cd4dddb7a2e5095"),
                felt!("0xbeef"),
            ],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![short_string!("Position"), felt!("0xbeef"), felt!("0")],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
        EmittedEvent {
            data: vec![
                short_string!("Moves"),
                felt!("0x07a3234437ebfadbae8465c1e81660c714e09bb77fa248ccfe66c6f3e6a03698"),
                felt!("0"),
            ],
            keys: vec![],
            block_hash: Default::default(),
            from_address: Default::default(),
            block_number: Default::default(),
            transaction_hash: Default::default(),
        },
    ];

    let actual_models = parse_registered_model_events(events);

    assert_eq!(actual_models.len(), 2);
    assert!(expected_models.contains(&actual_models[0]));
    assert!(expected_models.contains(&actual_models[1]));
}

#[test]
fn test_parse_deployed_contracts_events() {
    let expected_contracts = HashMap::from([
        (
            felt!("0x123"),
            Contract {
                name: "".into(),
                class_hash: felt!("0x1"),
                address: Some(felt!("0x123")),
                ..Default::default()
            },
        ),
        (
            felt!("0x456"),
            Contract {
                name: "".into(),
                class_hash: felt!("0x2"),
                address: Some(felt!("0x456")),
                ..Default::default()
            },
        ),
        (
            felt!("0x789"),
            Contract {
                name: "".into(),
                class_hash: felt!("0x3"),
                address: Some(felt!("0x789")),
                ..Default::default()
            },
        ),
    ]);

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

    let actual_contracts = parse_deployed_contracts_events(events);

    assert_eq!(actual_contracts, expected_contracts);
}
