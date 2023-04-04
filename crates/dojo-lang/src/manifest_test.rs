#![allow(unused)]
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

    let mut compiled_contracts: HashMap<SmolStr, FieldElement> = HashMap::new();
    compiled_contracts.insert("World".into(), FieldElement::from_hex_be("0x123").unwrap());
    compiled_contracts.insert("Store".into(), FieldElement::from_hex_be("0x123").unwrap());
    compiled_contracts.insert("Indexer".into(), FieldElement::from_hex_be("0x123").unwrap());
    compiled_contracts.insert("Executor".into(), FieldElement::from_hex_be("0x123").unwrap());
    compiled_contracts
        .insert("PositionComponent".into(), FieldElement::from_hex_be("0x123").unwrap());
    compiled_contracts.insert("MoveSystem".into(), FieldElement::from_hex_be("0x123").unwrap());

    let manifest = Manifest::new(db, &db.crates(), compiled_contracts);
    assert_eq!(manifest.components.len(), 1);
    assert_eq!(manifest.systems.len(), 1);
}
