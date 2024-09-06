use starknet::macros::felt;

use super::*;
use crate::contracts::naming::{get_filename_from_tag, get_tag};
use crate::manifest::{BaseManifest, Class, DojoContract, Manifest};

#[test]
fn no_diff_when_local_and_remote_are_equal() {
    let world_contract = Manifest::new(
        Class { class_hash: 66_u32.into(), ..Default::default() },
        get_filename_from_tag(WORLD_CONTRACT_TAG),
    );

    let base_contract = Manifest::new(
        Class { class_hash: 77_u32.into(), ..Default::default() },
        get_filename_from_tag(BASE_CONTRACT_TAG),
    );

    let local = BaseManifest { world: world_contract, base: base_contract, contracts: vec![] };

    let mut remote: DeploymentManifest = local.clone().into();
    remote.models = remote_models;

    let diff = WorldDiff::compute(local, Some(remote), "dojo-test").unwrap();

    assert_eq!(diff.count_diffs(), 0);
}

#[test]
fn diff_when_local_and_remote_are_different() {
    let world_contract = Manifest::new(
        Class { class_hash: 66_u32.into(), ..Default::default() },
        get_filename_from_tag(WORLD_CONTRACT_TAG),
    );

    let base_contract = Manifest::new(
        Class { class_hash: 77_u32.into(), ..Default::default() },
        get_filename_from_tag(BASE_CONTRACT_TAG),
    );

    let contracts = vec![
        Manifest::new(
            DojoContract {
                tag: get_tag("dojo_mock", "my_contract"),
                class_hash: felt!("0x1111"),
                address: Some(felt!("0x2222")),
                ..DojoContract::default()
            },
            get_filename_from_tag(&get_tag("dojo_mock", "my_contract")),
        ),
        Manifest::new(
            DojoContract {
                tag: get_tag("dojo_mock", "my_contract2"),
                class_hash: felt!("0x3333"),
                address: Some(felt!("4444")),
                ..DojoContract::default()
            },
            get_filename_from_tag(&get_tag("dojo_mock", "my_contract2")),
        ),
    ];

    let local = BaseManifest { models, contracts, world: world_contract, base: base_contract };

    let mut remote: DeploymentManifest = local.clone().into();
    remote.models = remote_models;
    remote.world.inner.class_hash = 44_u32.into();
    remote.models[1].inner.class_hash = 33_u32.into();
    remote.contracts[0].inner.class_hash = felt!("0x1112");

    let diff = WorldDiff::compute(local, Some(remote), "dojo-test").unwrap();

    assert_eq!(diff.count_diffs(), 3);
    assert!(diff.models.iter().any(|m| m.tag == get_tag("dojo_mock", "model2")));
    assert!(diff.contracts.iter().any(|c| c.tag == get_tag("dojo_mock", "my_contract")));
}

#[test]
fn updating_order_as_expected() {
    let init_calldata = vec![
        ("ns", "c4", vec!["$contract_address:ns-c1", "0x0"]),
        ("ns", "c3", vec!["0x0"]),
        ("ns", "c5", vec!["$contract_address:ns-c4", "0x0"]),
        ("ns", "c7", vec!["$contract_address:ns-c4", "0x0"]),
        ("ns", "c2", vec!["0x0"]),
        ("ns", "c6", vec!["$contract_address:ns-c4", "$contract_address:ns-c3", "0x0"]),
        ("ns", "c1", vec!["0x0"]),
    ];

    let mut contracts = vec![];
    for calldata in init_calldata {
        contracts.push(ContractDiff {
            init_calldata: calldata.2.iter().map(|c| c.to_string()).collect(),
            tag: get_tag(calldata.0, calldata.1),
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

    let expected_order = ["ns-c1", "ns-c2", "ns-c3", "ns-c4", "ns-c5", "ns-c6", "ns-c7"];
    for (i, contract) in diff.contracts.iter().enumerate() {
        assert_eq!(contract.tag, expected_order[i]);
    }
}

#[test]
fn updating_order_when_cyclic_dependency_fail() {
    let init_calldata = vec![
        ("ns", "c4", vec!["$contract_address:ns-c1", "$contract_address:ns-c6", "0x0"]),
        ("ns", "c3", vec!["0x0"]),
        ("ns", "c5", vec!["$contract_address:ns-c4", "0x0"]),
        ("ns", "c7", vec!["$contract_address:ns-c4", "0x0"]),
        ("ns", "c2", vec!["0x0"]),
        ("ns", "c6", vec!["$contract_address:ns-c4", "$contract_address:ns-c3", "0x0"]),
        ("ns", "c1", vec!["0x0"]),
    ];

    let mut contracts = vec![];
    for calldata in init_calldata {
        contracts.push(ContractDiff {
            init_calldata: calldata.2.iter().map(|c| c.to_string()).collect(),
            tag: get_tag(calldata.0, calldata.1),
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
