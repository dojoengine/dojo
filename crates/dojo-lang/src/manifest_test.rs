use std::collections::HashMap;

use assert_fs::TempDir;
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_semantic::test_utils::setup_test_crate;
use camino::{Utf8Path, Utf8PathBuf};
use pretty_assertions::assert_eq;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;
use smol_str::SmolStr;
use starknet::core::types::FieldElement;

use crate::compiler::DojoCompiler;
use crate::manifest::Manifest;
use crate::testing::build_test_db;

#[test]
fn test_manifest_generation() {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cache_dir = TempDir::new().unwrap();
    let config_dir = TempDir::new().unwrap();

    let path = Utf8PathBuf::from_path_buf("src/manifest_test_crate/Scarb.toml".into()).unwrap();
    let config = Config::builder(path.canonicalize_utf8().unwrap())
        .global_cache_dir_override(Some(Utf8Path::from_path(cache_dir.path()).unwrap()))
        .global_config_dir_override(Some(Utf8Path::from_path(config_dir.path()).unwrap()))
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(Some("scarb=trace"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();
    let db = &mut build_test_db(&ws).unwrap();
    let _crate_id = setup_test_crate(
        db,
        "
            #[derive(Component, Copy, Drop, Serde)]
            struct Position {
                x: usize,
                y: usize,
            }

            #[system]
            mod Move {
                fn execute() {}
            }
        ",
    );

    let class_hash = FieldElement::from_hex_be("0x123").unwrap();

    let mut compiled_contracts: HashMap<SmolStr, FieldElement> = HashMap::new();
    compiled_contracts.insert("World".into(), class_hash);
    compiled_contracts.insert("Executor".into(), class_hash);
    compiled_contracts
        .insert("PositionComponent".into(), FieldElement::from_hex_be("0x420").unwrap());
    compiled_contracts.insert("MoveSystem".into(), FieldElement::from_hex_be("0x69").unwrap());

    let manifest = Manifest::new(db, &db.crates(), compiled_contracts);

    assert_eq!(manifest.0.components.len(), 1);
    assert_eq!(manifest.0.systems.len(), 1);
    assert_eq!(manifest.0.world, class_hash);
    assert_eq!(manifest.0.executor, class_hash);
    assert_eq!(
        manifest.0.components.iter().find(|c| &c.name == "Position").unwrap().class_hash,
        FieldElement::from_hex_be("0x420").unwrap()
    );
    assert_eq!(
        manifest.0.systems.iter().find(|c| &c.name == "MoveSystem").unwrap().class_hash,
        FieldElement::from_hex_be("0x69").unwrap()
    );
}
