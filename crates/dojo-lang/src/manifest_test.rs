use std::collections::HashMap;

use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_semantic::test_utils::setup_test_crate;
use pretty_assertions::assert_eq;
use smol_str::SmolStr;
use starknet::core::types::FieldElement;

use crate::manifest::Manifest;
use crate::testing::build_test_db;

#[test]
fn test_manifest_generation() {
    let db = &mut build_test_db().unwrap();
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
