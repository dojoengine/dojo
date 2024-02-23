use starknet::macros::felt;

use super::*;
use crate::manifest::{BaseManifest, DojoContract, DojoModel};

#[test]
fn no_diff_when_local_and_remote_are_equal() {
    let world_contract = DojoContract {
        address: Some(77_u32.into()),
        class_hash: 66_u32.into(),
        name: WORLD_CONTRACT_NAME.into(),
        ..Default::default()
    };

    let models = vec![DojoModel {
        members: vec![],
        name: "dojo_mock::models::model".into(),
        class_hash: 11_u32.into(),
        ..Default::default()
    }];

    let remote_models = vec![DojoModel {
        members: vec![],
        name: "Model".into(),
        class_hash: 11_u32.into(),
        ..Default::default()
    }];

    let local = BaseManifest { models, world: world_contract, ..Default::default() };

    let mut remote = local.clone();
    remote.models = remote_models;

    let diff = WorldDiff::compute(local, Some(remote));

    assert_eq!(diff.count_diffs(), 0);
}

#[test]
fn diff_when_local_and_remote_are_different() {
    let world_contract = DojoContract {
        class_hash: 66_u32.into(),
        name: WORLD_CONTRACT_NAME.into(),
        ..Default::default()
    };

    let models = vec![
        DojoModel {
            members: vec![],
            name: "dojo_mock::models::model".into(),
            class_hash: felt!("0x11"),
            ..Default::default()
        },
        DojoModel {
            members: vec![],
            name: "dojo_mock::models::model_2".into(),
            class_hash: felt!("0x22"),
            ..Default::default()
        },
    ];

    let remote_models = vec![
        DojoModel {
            members: vec![],
            name: "Model".into(),
            class_hash: felt!("0x11"),
            ..Default::default()
        },
        DojoModel {
            members: vec![],
            name: "Model2".into(),
            class_hash: felt!("0x33"),
            ..Default::default()
        },
    ];

    let contracts = vec![
        DojoContract {
            name: "dojo_mock::contracts::my_contract".into(),
            class_hash: felt!("0x1111"),
            address: Some(felt!("0x2222")),
            ..DojoContract::default()
        },
        DojoContract {
            name: "dojo_mock::contracts::my_contract_2".into(),
            class_hash: felt!("0x3333"),
            address: Some(felt!("4444")),
            ..DojoContract::default()
        },
    ];

    let local = BaseManifest { models, contracts, world: world_contract, ..Default::default() };

    let mut remote = local.clone();
    remote.models = remote_models;
    remote.world.class_hash = 44_u32.into();
    remote.models[1].class_hash = 33_u32.into();
    remote.contracts[0].class_hash = felt!("0x1112");

    let diff = WorldDiff::compute(local, Some(remote));

    assert_eq!(diff.count_diffs(), 3);
    assert!(diff.models.iter().any(|m| m.name == "dojo_mock::models::model_2"));
    assert!(diff.contracts.iter().any(|c| c.name == "dojo_mock::contracts::my_contract"));
}
