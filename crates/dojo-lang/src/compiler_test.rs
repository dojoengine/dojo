use std::path::Path;
use std::{env, fs};

use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use dojo_test_utils::compiler::build_test_config;
use scarb::ops;

#[test]
fn test_compiler() {
    use dojo_test_utils::compiler::build_test_config;
    use scarb::ops;

    let config = build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let packages = ws.members().map(|p| p.id).collect();
    ops::compile(packages, &ws).unwrap_or_else(|op| panic!("Error compiling: {op:?}"))
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
