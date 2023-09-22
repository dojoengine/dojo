use super::*;
use crate::manifest::{Contract, Manifest, Model, System};

#[test]
fn no_diff_when_local_and_remote_are_equal() {
    let world_contract = Contract {
        address: Some(77_u32.into()),
        class_hash: 66_u32.into(),
        name: WORLD_CONTRACT_NAME.into(),
        ..Default::default()
    };

    let executor_contract = Contract {
        address: Some(88_u32.into()),
        class_hash: 99_u32.into(),
        name: EXECUTOR_CONTRACT_NAME.into(),
        ..Default::default()
    };

    let components = vec![Model {
        members: vec![],
        name: "Component".into(),
        class_hash: 11_u32.into(),
        ..Default::default()
    }];

    let systems =
        vec![System { name: "System".into(), class_hash: 22_u32.into(), ..Default::default() }];

    let local = Manifest {
        components,
        world: world_contract,
        executor: executor_contract,
        systems,
        ..Default::default()
    };
    let remote = local.clone();

    let diff = WorldDiff::compute(local, Some(remote));

    assert_eq!(diff.count_diffs(), 0);
}

#[test]
fn diff_when_local_and_remote_are_different() {
    let world_contract = Contract {
        class_hash: 66_u32.into(),
        name: WORLD_CONTRACT_NAME.into(),
        ..Default::default()
    };

    let executor_contract = Contract {
        class_hash: 99_u32.into(),
        name: EXECUTOR_CONTRACT_NAME.into(),
        ..Default::default()
    };

    let components = vec![Model {
        members: vec![],
        name: "Component".into(),
        class_hash: 11_u32.into(),
        ..Default::default()
    }];

    let systems =
        vec![System { name: "System".into(), class_hash: 22_u32.into(), ..Default::default() }];

    let local = Manifest {
        components,
        world: world_contract,
        executor: executor_contract,
        systems,
        ..Default::default()
    };

    let mut remote = local.clone();
    remote.world.class_hash = 44_u32.into();
    remote.executor.class_hash = 55_u32.into();
    remote.components[0].class_hash = 33_u32.into();

    let diff = WorldDiff::compute(local, Some(remote));

    assert_eq!(diff.count_diffs(), 3);
}
