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
