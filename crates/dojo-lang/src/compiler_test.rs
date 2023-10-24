use std::ops::Deref;
use std::path::Path;
use std::{env, fs};

use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use dojo_test_utils::compiler::build_test_config;
use scarb::ops;
use starknet::macros::felt;

#[test]
fn test_compiler() {
    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();
    let packages = ws.members().map(|p| p.id).collect();
    let dojo_manifest_path = format!("{}/manifest.json", config.profile().as_str());

    assert!(ops::compile(packages, &ws).is_ok(), "compilation failed");

    let mut manifest = config
        .target_dir()
        .open_ro(&dojo_manifest_path, "output file", ws.config())
        .map(|file| dojo_world::manifest::Manifest::try_from(file.deref()).unwrap_or_default())
        .unwrap();

    let world_address = Some(felt!("0xbeef"));
    manifest.world.address = world_address;

    manifest
        .write_to_path(
            config
                .target_dir()
                .open_rw(&dojo_manifest_path, "output file", ws.config())
                .unwrap()
                .path(),
        )
        .unwrap();

    let manifest = config
        .target_dir()
        .open_ro(dojo_manifest_path, "output file", ws.config())
        .map(|file| dojo_world::manifest::Manifest::try_from(file.deref()).unwrap_or_default())
        .unwrap();

    assert_eq!(manifest.world.address, world_address, "manifest should be fully overritten");
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
) -> Result<OrderedHashMap<String, String>, String> {
    let config = build_test_config("./src/manifest_test_data/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();

    let packages = ws.members().map(|p| p.id).collect();
    ops::compile(packages, &ws).unwrap_or_else(|op| panic!("Error compiling: {op:?}"));

    let target_dir = config.target_dir().path_existent().unwrap();

    let generated_manifest_path =
        Path::new(target_dir).join(config.profile().as_str()).join("manifest.json");

    let generated_file = fs::read_to_string(generated_manifest_path).unwrap();

    Ok(OrderedHashMap::from([("expected_manifest_file".into(), generated_file)]))
}
