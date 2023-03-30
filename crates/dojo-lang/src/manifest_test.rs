use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_semantic::test_utils::setup_test_crate;
use pretty_assertions::assert_eq;

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

    let manifest = Manifest::new(db, &db.crates());
    assert_eq!(manifest.components.len(), 1);
    assert_eq!(manifest.systems.len(), 1);
}
