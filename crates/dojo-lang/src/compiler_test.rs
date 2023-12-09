use std::collections::BTreeMap;
use std::path::Path;
use std::{env, fs};

use cairo_lang_test_utils::parse_test_file::TestRunnerResult;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use dojo_test_utils::compiler::build_test_config;
use dojo_world::manifest::{BASE_CONTRACT_NAME, EXECUTOR_CONTRACT_NAME, WORLD_CONTRACT_NAME};
use scarb::core::TargetKind;
use scarb::ops::CompileOpts;
use smol_str::SmolStr;
use starknet::macros::felt;

use super::do_update_manifest;
use crate::scarb_internal::{self};

fn build_mock_manifest() -> dojo_world::manifest::Manifest {
    dojo_world::manifest::Manifest {
        world: dojo_world::manifest::Contract {
            name: WORLD_CONTRACT_NAME.into(),
            abi: None,
            address: Some(felt!("0xbeef")),
            class_hash: felt!("0xdeadbeef"),
            ..Default::default()
        },
        executor: dojo_world::manifest::Contract {
            name: EXECUTOR_CONTRACT_NAME.into(),
            abi: None,
            address: Some(felt!("0x1234")),
            class_hash: felt!("0x4567"),
            ..Default::default()
        },
        base: dojo_world::manifest::Class {
            name: BASE_CONTRACT_NAME.into(),
            class_hash: felt!("0x9090"),
            ..Default::default()
        },
        contracts: vec![
            dojo_world::manifest::Contract {
                name: "TestContract1".into(),
                abi: None,
                address: Some(felt!("0x1111")),
                class_hash: felt!("0x2222"),
                ..Default::default()
            },
            dojo_world::manifest::Contract {
                name: "TestContract2".into(),
                abi: None,
                address: Some(felt!("0x3333")),
                class_hash: felt!("0x4444"),
                ..Default::default()
            },
        ],
        models: vec![
            dojo_world::manifest::Model {
                name: "TestModel1".into(),
                class_hash: felt!("0x5555"),
                ..Default::default()
            },
            dojo_world::manifest::Model {
                name: "TestModel2".into(),
                class_hash: felt!("0x66666"),
                ..Default::default()
            },
            dojo_world::manifest::Model {
                name: "TestModel3".into(),
                class_hash: felt!("0x7777"),
                ..Default::default()
            },
        ],
    }
}

#[test]
fn update_manifest_correctly() {
    let mut mock_manifest = build_mock_manifest();

    let world = mock_manifest.world.clone();
    let executor = mock_manifest.executor.clone();
    let base = mock_manifest.base.clone();
    let contracts = mock_manifest.contracts.clone();

    let new_models: BTreeMap<String, dojo_world::manifest::Model> = [(
        "TestModel3000".into(),
        dojo_world::manifest::Model {
            name: "TestModel3000".into(),
            class_hash: felt!("0x3000"),
            ..Default::default()
        },
    )]
    .into();

    let new_contracts: BTreeMap<SmolStr, dojo_world::manifest::Contract> = [
        (
            "TestContract1".into(),
            dojo_world::manifest::Contract {
                name: "TestContract1".into(),
                abi: None,
                class_hash: felt!("0x2211"),
                ..Default::default()
            },
        ),
        (
            "TestContract2".into(),
            dojo_world::manifest::Contract {
                name: "TestContract2".into(),
                abi: None,
                class_hash: felt!("0x4411"),
                ..Default::default()
            },
        ),
        (
            "TestContract3".into(),
            dojo_world::manifest::Contract {
                name: "TestContract3".into(),
                abi: None,
                class_hash: felt!("0x0808"),
                ..Default::default()
            },
        ),
    ]
    .into();

    do_update_manifest(
        &mut mock_manifest,
        world.clone(),
        executor.clone(),
        base.clone(),
        new_models.clone(),
        new_contracts,
    )
    .unwrap();

    assert!(mock_manifest.world == world, "world should not change");
    assert!(mock_manifest.executor == executor, "executor should not change");
    assert!(mock_manifest.base == base, "base should not change");

    assert!(mock_manifest.models == new_models.into_values().collect::<Vec<_>>());

    assert!(mock_manifest.contracts.len() == 3);

    assert!(
        mock_manifest.contracts[0].address == contracts[0].address,
        "contract address should not change"
    );
    assert!(
        mock_manifest.contracts[1].address == contracts[1].address,
        "contract address should not change"
    );
    assert!(mock_manifest.contracts[2].address.is_none(), "new contract do not have address");
}

#[test]
fn test_compiler() {
    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    assert!(
        scarb_internal::compile_workspace(
            &config,
            CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
        )
        .is_ok(),
        "compilation failed"
    );
}

cairo_lang_test_utils::test_file_test!(
    manifest_file,
    "src/manifest_test_data/",
    {
        manifest: "manifest",
    },
    test_manifest_file
);

pub fn test_manifest_file(
    _inputs: &OrderedHashMap<String, String>,
    _args: &OrderedHashMap<String, String>,
) -> TestRunnerResult {
    let config = build_test_config("./src/manifest_test_data/spawn-and-move/Scarb.toml").unwrap();

    scarb_internal::compile_workspace(
        &config,
        CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
    )
    .unwrap_or_else(|err| panic!("Error compiling: {err:?}"));

    let target_dir = config.target_dir_override().unwrap();

    let generated_manifest_path =
        Path::new(target_dir).join(config.profile().as_str()).join("manifest.json");

    let generated_file = fs::read_to_string(generated_manifest_path).unwrap();

    TestRunnerResult::success(OrderedHashMap::from([(
        "expected_manifest_file".into(),
        generated_file,
    )]))
}

cairo_lang_test_utils::test_file_test!(
    compiler_cairo_v240,
    "src/manifest_test_data/",
    {
        cairo_v240: "cairo_v240",
    },
    test_compiler_cairo_v240
);

pub fn test_compiler_cairo_v240(
    _inputs: &OrderedHashMap<String, String>,
    _args: &OrderedHashMap<String, String>,
) -> TestRunnerResult {
    let config =
        build_test_config("./src/manifest_test_data/compiler_cairo_v240/Scarb.toml").unwrap();

    scarb_internal::compile_workspace(
        &config,
        CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
    )
    .unwrap_or_else(|err| panic!("Error compiling: {err:?}"));

    let target_dir = config.target_dir_override().unwrap();

    let generated_manifest_path =
        Path::new(target_dir).join(config.profile().as_str()).join("manifest.json");

    let generated_file = fs::read_to_string(generated_manifest_path).unwrap();

    TestRunnerResult::success(OrderedHashMap::from([(
        "expected_manifest_file".into(),
        generated_file,
    )]))
}
