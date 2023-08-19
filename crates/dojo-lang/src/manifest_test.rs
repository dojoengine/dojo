use std::collections::HashMap;
use std::env;

use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use dojo_test_utils::compiler::build_test_db;
use smol_str::SmolStr;
use starknet::core::types::FieldElement;

use crate::manifest::Manifest;

cairo_lang_test_utils::test_file_test!(
    manifest_file,
    "src/manifest_test_crate",
    {
        manifest: "manifest",
    },
    test_manifest_file
);

pub fn test_manifest_file(
    inputs: &OrderedHashMap<String, String>,
) -> OrderedHashMap<String, String> {
    let db = &mut build_test_db("../../examples/ecs/Scarb.toml").unwrap();
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

    dbg!(&manifest);
    todo!("Compare generated manifest file with expected one in inputs");
}
