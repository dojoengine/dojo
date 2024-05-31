use starknet::macros::felt;

use super::*;
use crate::manifest::{BaseManifest, Class, DojoContract, DojoModel, Manifest};

#[test]
fn no_diff_when_local_and_remote_are_equal() {
    let world_contract = Manifest::new(
        Class { class_hash: 66_u32.into(), ..Default::default() },
        WORLD_CONTRACT_NAME.into(),
        WORLD_CONTRACT_NAME.into(),
    );

    let base_contract = Manifest::new(
        Class { class_hash: 77_u32.into(), ..Default::default() },
        BASE_CONTRACT_NAME.into(),
        BASE_CONTRACT_NAME.into(),
    );

    let models = vec![Manifest::new(
        DojoModel { members: vec![], class_hash: 11_u32.into(), ..Default::default() },
        "dojo_mock_model".into(),
        "dojo_mock::models::model".into(),
    )];

    let remote_models = vec![Manifest::new(
        DojoModel { members: vec![], class_hash: 11_u32.into(), ..Default::default() },
        "dojo_mock_model".into(),
        "dojo_mock::models::model".into(),
    )];

    let local =
        BaseManifest { models, world: world_contract, base: base_contract, contracts: vec![] };

    let mut remote: DeploymentManifest = local.clone().into();
    remote.models = remote_models;

    let diff = WorldDiff::compute(local, Some(remote));

    println!("{:?}", diff);

    assert_eq!(diff.count_diffs(), 0);
}

#[test]
fn diff_when_local_and_remote_are_different() {
    let world_contract = Manifest::new(
        Class { class_hash: 66_u32.into(), ..Default::default() },
        WORLD_CONTRACT_NAME.into(),
        WORLD_CONTRACT_NAME.into(),
    );

    let base_contract = Manifest::new(
        Class { class_hash: 77_u32.into(), ..Default::default() },
        BASE_CONTRACT_NAME.into(),
        BASE_CONTRACT_NAME.into(),
    );

    let models = vec![
        Manifest::new(
            DojoModel {
                name: "model".to_string(),
                namespace: "dojo_mock".to_string(),
                members: vec![],
                class_hash: felt!("0x11"),
                ..Default::default()
            },
            "dojo_mock:model".into(),
            "dojo_mock::models::model".into(),
        ),
        Manifest::new(
            DojoModel {
                name: "model_2".to_string(),
                namespace: "dojo_mock".to_string(),
                members: vec![],
                class_hash: felt!("0x22"),
                ..Default::default()
            },
            "dojo_mock:model_2".into(),
            "dojo_mock::models::model_2".into(),
        ),
    ];

    let remote_models = vec![
        Manifest::new(
            DojoModel {
                name: "model".to_string(),
                namespace: "dojo_mock".to_string(),
                members: vec![],
                class_hash: felt!("0x11"),
                ..Default::default()
            },
            "dojo_mock:model".into(),
            "dojo_mock::models::model".into(),
        ),
        Manifest::new(
            DojoModel {
                name: "model_2".to_string(),
                namespace: "dojo_mock".to_string(),
                members: vec![],
                class_hash: felt!("0x33"),
                ..Default::default()
            },
            "dojo_mock:model_2".into(),
            "dojo_mock::models::model_2".into(),
        ),
    ];

    let contracts = vec![
        Manifest::new(
            DojoContract {
                name: "my_contract".to_string(),
                namespace: "dojo_mock".to_string(),
                class_hash: felt!("0x1111"),
                address: Some(felt!("0x2222")),
                ..DojoContract::default()
            },
            "dojo_mock:my_contract".into(),
            "dojo_mock::contracts::my_contract".into(),
        ),
        Manifest::new(
            DojoContract {
                name: "my_contract_2".to_string(),
                namespace: "dojo_mock".to_string(),
                class_hash: felt!("0x3333"),
                address: Some(felt!("4444")),
                ..DojoContract::default()
            },
            "dojo_mock:my_contract_2".into(),
            "dojo_mock::contracts::my_contract_2".into(),
        ),
    ];

    let local = BaseManifest { models, contracts, world: world_contract, base: base_contract };

    let mut remote: DeploymentManifest = local.clone().into();
    remote.models = remote_models;
    remote.world.inner.class_hash = 44_u32.into();
    remote.models[1].inner.class_hash = 33_u32.into();
    remote.contracts[0].inner.class_hash = felt!("0x1112");

    let diff = WorldDiff::compute(local, Some(remote));

    assert_eq!(diff.count_diffs(), 3);
    assert!(diff.models.iter().any(|m| m.name == "model_2" && m.namespace == "dojo_mock"));
    assert!(diff.contracts.iter().any(|c| c.name == "my_contract" && c.namespace == "dojo_mock"));
}

#[test]
fn updating_order_as_expected() {
    let init_calldata = vec![
        ("ns", "c4", vec!["$contract_address:ns:c1", "0x0"]),
        ("ns", "c3", vec!["0x0"]),
        ("ns", "c5", vec!["$contract_address:ns:c4", "0x0"]),
        ("ns", "c7", vec!["$contract_address:ns:c4", "0x0"]),
        ("ns", "c2", vec!["0x0"]),
        ("ns", "c6", vec!["$contract_address:ns:c4", "$contract_address:ns:c3", "0x0"]),
        ("ns", "c1", vec!["0x0"]),
    ];

    let mut contracts = vec![];
    for calldata in init_calldata {
        contracts.push(ContractDiff {
            init_calldata: calldata.2.iter().map(|c| c.to_string()).collect(),
            name: calldata.1.to_string(),
            namespace: calldata.0.to_string(),
            ..Default::default()
        });
    }

    let mut diff = WorldDiff {
        world: ContractDiff::default(),
        base: ClassDiff::default(),
        contracts,
        models: vec![],
    };

    diff.update_order("ns").unwrap();

    let expected_order = ["c1", "c2", "c3", "c4", "c5", "c6", "c7"];
    for (i, contract) in diff.contracts.iter().enumerate() {
        assert_eq!(contract.name, expected_order[i]);
    }
}

#[test]
fn updating_order_when_cyclic_dependency_fail() {
    let init_calldata = vec![
        ("ns", "c4", vec!["$contract_address:ns:c1", "$contract_address:ns:c6", "0x0"]),
        ("ns", "c3", vec!["0x0"]),
        ("ns", "c5", vec!["$contract_address:ns:c4", "0x0"]),
        ("ns", "c7", vec!["$contract_address:ns:c4", "0x0"]),
        ("ns", "c2", vec!["0x0"]),
        ("ns", "c6", vec!["$contract_address:ns:c4", "$contract_address:ns:c3", "0x0"]),
        ("ns", "c1", vec!["0x0"]),
    ];

    let mut contracts = vec![];
    for calldata in init_calldata {
        contracts.push(ContractDiff {
            init_calldata: calldata.2.iter().map(|c| c.to_string()).collect(),
            namespace: calldata.0.to_string(),
            name: calldata.1.to_string(),
            ..Default::default()
        });
    }

    let mut diff = WorldDiff {
        world: ContractDiff::default(),
        base: ClassDiff::default(),
        contracts,
        models: vec![],
    };

    assert!(diff.update_order("ns").is_err_and(|e| e.to_string().contains("Cyclic")));
}
