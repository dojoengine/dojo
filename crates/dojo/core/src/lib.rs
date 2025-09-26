// Placeholder so we can use `cargo release` to update Scarb.toml

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn world_version_constant_matches_workspace_version() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let contract_path = Path::new(manifest_dir).join("src/world/world_contract.cairo");
        let file = fs::read_to_string(contract_path).expect("failed to read world_contract.cairo");

        let needle = "pub const WORLD_VERSION: felt252 = '";
        let Some(start) = file.find(needle) else {
            panic!("WORLD_VERSION constant not found in world_contract.cairo");
        };
        let remainder = &file[start + needle.len()..];
        let Some((cairo_version, _)) = remainder.split_once("'") else {
            panic!("WORLD_VERSION constant is malformed");
        };
        let cargo_version = env!("CARGO_PKG_VERSION");

        assert_eq!(
            cairo_version, cargo_version,
            "world contract version ({cairo_version}) does not match Cargo package version \
             ({cargo_version})",
        );
    }
}
