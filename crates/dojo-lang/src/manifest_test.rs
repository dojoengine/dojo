use std::collections::HashMap;
use std::env;

use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_semantic::test_utils::setup_test_crate;
use camino::Utf8PathBuf;
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

    let path = Utf8PathBuf::from_path_buf("src/manifest_test_crate/Scarb.toml".into()).unwrap();
    let config = Config::builder(path.canonicalize_utf8().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();
    let db = &mut build_test_db(&ws).unwrap();
    let _crate_id = setup_test_crate(
        db,
        "
            #[derive(Component)]
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
    compiled_contracts.insert("Store".into(), class_hash);
    compiled_contracts.insert("Indexer".into(), class_hash);
    compiled_contracts.insert("Executor".into(), class_hash);
    compiled_contracts
        .insert("PositionComponent".into(), FieldElement::from_hex_be("0x420").unwrap());
    compiled_contracts.insert("MoveSystem".into(), FieldElement::from_hex_be("0x69").unwrap());

    let manifest = Manifest::new(db, &db.crates(), compiled_contracts);

    assert_eq!(manifest.components.len(), 1);
    assert_eq!(manifest.systems.len(), 1);
    assert_eq!(manifest.world.unwrap(), class_hash);
    assert_eq!(manifest.executor.unwrap(), class_hash);
    assert_eq!(manifest.indexer.unwrap(), class_hash);
    assert_eq!(manifest.store.unwrap(), class_hash);
    assert_eq!(
        manifest.components.iter().find(|c| &c.name == "Position").unwrap().class_hash,
        FieldElement::from_hex_be("0x420").unwrap()
    );
    assert_eq!(
        manifest.systems.iter().find(|c| &c.name == "MoveSystem").unwrap().class_hash,
        FieldElement::from_hex_be("0x69").unwrap()
    );
}
