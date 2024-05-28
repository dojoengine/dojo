use starknet::macros::felt;

use super::*;
use crate::manifest::{BaseManifest, Class, DojoContract, DojoModel, Manifest};

#[test]
fn no_diff_when_local_and_remote_are_equal() {
    let world_contract = Manifest::new(
        Class { class_hash: 66_u32.into(), ..Default::default() },
        WORLD_CONTRACT_NAME.into(),
    );

    let base_contract = Manifest::new(
        Class { class_hash: 77_u32.into(), ..Default::default() },
        BASE_CONTRACT_NAME.into(),
    );

    let models = vec![Manifest::new(
        DojoModel { members: vec![], class_hash: 11_u32.into(), ..Default::default() },
        "dojo_mock::models::model".into(),
    )];

    let remote_models = vec![Manifest::new(
        DojoModel { members: vec![], class_hash: 11_u32.into(), ..Default::default() },
        "Model".into(),
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
    );

    let base_contract = Manifest::new(
        Class { class_hash: 77_u32.into(), ..Default::default() },
        BASE_CONTRACT_NAME.into(),
    );

    let models = vec![
        Manifest::new(
            DojoModel { members: vec![], class_hash: felt!("0x11"), ..Default::default() },
            "dojo_mock::models::model".into(),
        ),
        Manifest::new(
            DojoModel { members: vec![], class_hash: felt!("0x22"), ..Default::default() },
            "dojo_mock::models::model_2".into(),
        ),
    ];

    let remote_models = vec![
        Manifest::new(
            DojoModel { members: vec![], class_hash: felt!("0x11"), ..Default::default() },
            "Model".into(),
        ),
        Manifest::new(
            DojoModel { members: vec![], class_hash: felt!("0x33"), ..Default::default() },
            "Model2".into(),
        ),
    ];

    let contracts = vec![
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x1111"),
                address: Some(felt!("0x2222")),
                ..DojoContract::default()
            },
            "dojo_mock::contracts::my_contract".into(),
        ),
        Manifest::new(
            DojoContract {
                class_hash: felt!("0x3333"),
                address: Some(felt!("4444")),
                ..DojoContract::default()
            },
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
    assert!(diff.models.iter().any(|m| m.name == "dojo_mock::models::model_2"));
    assert!(diff.contracts.iter().any(|c| c.name == "dojo_mock::contracts::my_contract"));
}

#[test]
fn updating_order_as_expected() {
    let init_calldata = vec![
        ("c4", vec!["$contract_address:c1", "0x0"]),
        ("c3", vec!["0x0"]),
        ("c5", vec!["$contract_address:c4", "0x0"]),
        ("c7", vec!["$contract_address:c4", "0x0"]),
        ("c2", vec!["0x0"]),
        ("c6", vec!["$contract_address:c4", "$contract_address:c3", "0x0"]),
        ("c1", vec!["0x0"]),
    ];

    let mut contracts = vec![];
    for calldata in init_calldata {
        contracts.push(ContractDiff {
            init_calldata: calldata.1.iter().map(|c| c.to_string()).collect(),
            name: calldata.0.to_string(),
            ..Default::default()
        });
    }

    let mut diff = WorldDiff {
        world: ContractDiff::default(),
        base: ClassDiff::default(),
        contracts,
        models: vec![],
    };

    diff.update_order().unwrap();

    let expected_order = ["c1", "c2", "c3", "c4", "c5", "c6", "c7"];
    for (i, contract) in diff.contracts.iter().enumerate() {
        assert_eq!(contract.name, expected_order[i]);
    }
}

#[test]
fn updating_order_when_cyclic_dependency_fail() {
    let init_calldata = vec![
        ("c4", vec!["$contract_address:c1", "$contract_address:c6", "0x0"]),
        ("c3", vec!["0x0"]),
        ("c5", vec!["$contract_address:c4", "0x0"]),
        ("c7", vec!["$contract_address:c4", "0x0"]),
        ("c2", vec!["0x0"]),
        ("c6", vec!["$contract_address:c4", "$contract_address:c3", "0x0"]),
        ("c1", vec!["0x0"]),
    ];

    let mut contracts = vec![];
    for calldata in init_calldata {
        contracts.push(ContractDiff {
            init_calldata: calldata.1.iter().map(|c| c.to_string()).collect(),
            name: calldata.0.to_string(),
            ..Default::default()
        });
    }

    let mut diff = WorldDiff {
        world: ContractDiff::default(),
        base: ClassDiff::default(),
        contracts,
        models: vec![],
    };

    assert!(diff.update_order().is_err_and(|e| e.to_string().contains("Cyclic")));
}
