use std::env;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use cairo_proof_parser::output::extract_output;
use katana_primitives::contract::ContractAddress;
use katana_primitives::state::StateUpdates;
use katana_primitives::{address, felt};
use saya_core::prover::extract::program_input_from_program_output;
use saya_core::prover::{
    prove_diff, HttpProverParams, MessageToAppchain, MessageToStarknet, ProgramInput, ProveProgram,
    ProverIdentifier, ProvingState, Scheduler,
};
use saya_core::ProverAccessKey;
use starknet_crypto::Felt;
use tokio::time::sleep;

fn prover_identifier() -> ProverIdentifier {
    let prover_key = env::var("PROVER_ACCESS_KEY").expect("PROVER_ACCESS_KEY not set.");

    ProverIdentifier::Http(Arc::new(HttpProverParams {
        prover_url: "http://prover.visoft.dev:3618".parse().unwrap(),
        prover_key: ProverAccessKey::from_hex_string(&prover_key)
            .expect("Failed to parse prover key."),
    }))
}

fn sorted<E>(mut v: Vec<E>) -> Vec<E>
where
    E: std::cmp::Ord,
{
    v.sort();
    v
}

#[ignore]
#[tokio::test]
async fn test_program_input_from_program_output() -> anyhow::Result<()> {
    let mut input = ProgramInput {
        prev_state_root: Felt::from_str("101").unwrap(),
        block_number: 102,
        block_hash: Felt::from_str("103").unwrap(),
        config_hash: Felt::from_str("104").unwrap(),
        message_to_starknet_segment: vec![
            MessageToStarknet {
                from_address: address!("105"),
                to_address: address!("106"),
                payload: vec![Felt::from_str("107").unwrap()],
            },
            MessageToStarknet {
                from_address: address!("105"),
                to_address: address!("106"),
                payload: vec![Felt::from_str("107").unwrap()],
            },
        ],
        message_to_appchain_segment: vec![
            MessageToAppchain {
                from_address: address!("108"),
                to_address: address!("109"),
                nonce: Felt::from_str("110").unwrap(),
                selector: Felt::from_str("111").unwrap(),
                payload: vec![Felt::from_str("112").unwrap()],
            },
            MessageToAppchain {
                from_address: address!("108"),
                to_address: address!("109"),
                nonce: Felt::from_str("110").unwrap(),
                selector: Felt::from_str("111").unwrap(),
                payload: vec![Felt::from_str("112").unwrap()],
            },
        ],
        state_updates: StateUpdates {
            nonce_updates: {
                let mut map = std::collections::BTreeMap::new();
                map.insert(address!("1111"), felt!("22222"));
                map
            },
            storage_updates: vec![(
                address!("333"),
                vec![(Felt::from_str("4444")?, Felt::from_str("555")?)].into_iter().collect(),
            )]
            .into_iter()
            .collect(),
            deployed_contracts: {
                let mut map = std::collections::BTreeMap::new();
                map.insert(address!("66666"), felt!("7777"));
                map
            },
            declared_classes: {
                let mut map = std::collections::BTreeMap::new();
                map.insert(Felt::from_str("88888").unwrap(), Felt::from_str("99999").unwrap());
                map
            },
            ..Default::default()
        },
        world_da: None,
    };

    input.fill_da(333u64.into());

    let serialized_input = serde_json::to_string(&input).unwrap();
    let proof =
        prove_diff(serialized_input, prover_identifier(), ProveProgram::Differ).await.unwrap();

    let program_output_from_proof = extract_output(&proof).unwrap().program_output;
    let program_input_from_proof = program_input_from_program_output(
        program_output_from_proof,
        input.clone().state_updates,
        333u64.into(),
    )
    .unwrap();
    assert_eq!(input, program_input_from_proof);
    Ok(())
}

#[ignore]
#[tokio::test]
async fn test_combine_proofs() {
    let input1 = r#"{
        "prev_state_root": "101",
        "block_number": 102,
        "block_hash": "103",
        "config_hash": "104",
        "message_to_starknet_segment": [
            "105",
            "106",
            "1",
            "107"
        ],
        "message_to_appchain_segment": [
            "108",
            "109",
            "110",
            "111",
            "1",
            "112"
        ],
        "nonce_updates": {
            "1111": "22222"
        },
        "storage_updates": {
            "333": {
                "4444": "555"
            }
        },
        "contract_updates": {
            "66666": "7777"
        },
        "declared_classes": {
            "88888": "99999"
        },
        "world_da": []
    }"#;
    let input2 = r#"{
        "prev_state_root": "201",
        "block_number": 103,
        "block_hash": "203",
        "config_hash": "204",
        "message_to_starknet_segment": [
            "205",
            "206",
            "1",
            "207"
        ],
        "message_to_appchain_segment": [
            "208",
            "209",
            "210",
            "211",
            "1",
            "207"
        ],
        "nonce_updates": {
            "12334": "214354"
        },
        "storage_updates": {
            "333": {
                "44536346444": "565474555"
            }
        },
        "contract_updates": {
            "4356345": "775468977"
        },
        "declared_classes": {
            "88556753888": "9995764599"
        },
        "world_da": []
    }"#;
    let expected = r#"{
        "prev_state_root": "101",
        "block_number": 103,
        "block_hash": "203",
        "config_hash": "104",
        "message_to_starknet_segment": [
            "105",
            "106",
            "1",
            "107",
            "205",
            "206",
            "1",
            "207"
        ],
        "message_to_appchain_segment": [
            "108",
            "109",
            "110",
            "111",
            "1",
            "112",
            "208",
            "209",
            "210",
            "211",
            "1",
            "207"
        ],
        "nonce_updates": {
            "12334": "214354",
            "1111": "22222"
        },
        "storage_updates": {
            "333": {
                "44536346444": "565474555",
                "4444": "555"
            }
        },
        "contract_updates": {
            "4356345": "775468977",
            "66666": "7777"
        },
        "declared_classes": {
            "88556753888": "9995764599",
            "88888": "99999"
        },
        "world_da": [
            "4444",
            "555",
            "44536346444",
            "565474555"
        ]
    }"#;

    let mut inputs = vec![input1, input2]
        .into_iter()
        .map(|s| serde_json::from_str::<ProgramInput>(s).unwrap())
        .collect::<Vec<_>>();

    let world = Felt::from_dec_str("333").unwrap();
    for input in &mut inputs {
        input.fill_da(world)
    }

    let mut scheduler = Scheduler::new(2, world, prover_identifier());
    scheduler.push_diff(inputs.remove(0)).unwrap();

    sleep(Duration::from_millis(5)).await;

    assert!(!scheduler.is_full());
    assert_eq!(scheduler.query(102).await.unwrap(), ProvingState::Proving);
    assert_eq!(scheduler.query(103).await.unwrap(), ProvingState::NotPushed);

    scheduler.push_diff(inputs.remove(0)).unwrap();
    sleep(Duration::from_millis(5)).await;

    assert!(scheduler.is_full());
    assert_eq!(scheduler.query(102).await.unwrap(), ProvingState::Proving);
    assert_eq!(scheduler.query(103).await.unwrap(), ProvingState::Proving);

    let (_, output, block_range) = scheduler.proved().await.unwrap();
    let expected: ProgramInput = serde_json::from_str(expected).unwrap();
    assert_eq!(output, expected);
    assert_eq!(block_range, (102, 103));
}

#[ignore]
#[tokio::test]
async fn test_4_combine_proofs() -> anyhow::Result<()> {
    let world = Felt::from_dec_str("42")?;

    let input_1 = r#"{
        "prev_state_root": "101",
        "block_number": 101,
        "block_hash": "103",
        "config_hash": "104",
        "message_to_starknet_segment": ["105", "106", "1", "1"],
        "message_to_appchain_segment": ["108", "109", "110", "111", "1", "112"],
        "storage_updates": {
            "42": {
                "2010": "1200",
                "2012": "1300"
            }
        },
        "nonce_updates": {},
        "contract_updates": {},
        "declared_classes": {}
    }
    "#;

    let input_2 = r#"{
        "prev_state_root": "1011",
        "block_number": 102,
        "block_hash": "1033",
        "config_hash": "104",
        "message_to_starknet_segment": ["135", "136", "1", "1"],
        "message_to_appchain_segment": ["158", "159", "150", "151", "1", "152"],
        "storage_updates": {
            "42": {
                "2010": "1250",
                "2032": "1300"
            }
        },
        "nonce_updates": {},
        "contract_updates": {},
        "declared_classes": {}
    }"#;

    let input_3 = r#"{
        "prev_state_root": "10111",
        "block_number": 103,
        "block_hash": "10333",
        "config_hash": "104",
        "message_to_starknet_segment": [],
        "message_to_appchain_segment": [],
        "storage_updates": {
            "42": {
                "2013": "2"
            }
        },
        "nonce_updates": {},
        "contract_updates": {},
        "declared_classes": {}
    }"#;

    let input_4 = r#"{
        "prev_state_root": "101111",
        "block_number": 104,
        "block_hash": "103333",
        "config_hash": "104",
        "message_to_starknet_segment": ["165", "166", "1", "1"],
        "message_to_appchain_segment": ["168", "169", "160", "161", "1", "162"],
        "storage_updates": {
            "42": {
                "2010": "1700"
            }
        },
        "nonce_updates": {},
        "contract_updates": {},
        "declared_classes": {}
    }
    "#;

    let expected = r#"{
        "prev_state_root": "101",
        "block_number": 104,
        "block_hash": "103333",
        "config_hash": "104",
        "message_to_starknet_segment": ["105", "106", "1", "1", "135", "136", "1", "1", "165", "166", "1", "1"],
        "message_to_appchain_segment": ["108", "109", "110", "111", "1", "112", "158", "159", "150", "151", "1", "152", "168", "169", "160", "161", "1", "162"],
        "storage_updates": {
            "42": {
                "2010": "1700",
                "2012": "1300",
                "2032": "1300",
                "2013": "2"
            }
        },
        "nonce_updates": {},
        "contract_updates": {},
        "declared_classes": {},
        "world_da": ["2010", "1700", "2012", "1300", "2032", "1300", "2013", "2"]
    }"#;

    let inputs = vec![input_1, input_2, input_3, input_4]
        .into_iter()
        .map(|input| {
            let mut input = serde_json::from_str::<ProgramInput>(input).unwrap();
            input.fill_da(world);
            input
        })
        .collect::<Vec<_>>();

    let expected = serde_json::from_str::<ProgramInput>(expected).unwrap();

    let (_proof, output) = Scheduler::merge(inputs, world, prover_identifier()).await?;
    assert_eq!(output.message_to_appchain_segment, expected.message_to_appchain_segment);
    assert_eq!(output.message_to_starknet_segment, expected.message_to_starknet_segment);

    assert_eq!(sorted(output.world_da.unwrap()), sorted(expected.world_da.unwrap()));
    assert_eq!(output.state_updates, expected.state_updates);

    assert_eq!(expected.prev_state_root, output.prev_state_root);
    assert_eq!(expected.block_number, output.block_number);
    assert_eq!(expected.block_hash, output.block_hash);
    assert_eq!(expected.config_hash, output.config_hash);

    Ok(())
}
