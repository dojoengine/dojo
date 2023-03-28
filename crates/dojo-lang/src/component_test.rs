use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_semantic::test_utils::setup_test_crate;
use pretty_assertions::assert_eq;

use crate::component::find_components;
use crate::testing::build_test_db;

#[test]
fn test_component_resolving() {
    let db = &mut build_test_db().unwrap();

    let _crate_id = setup_test_crate(
        db,
        "
            mod NotAComponent {}

            #[derive(Component)]
            struct Position {
                x: usize,
                y: usize,
            }
        ",
    );

    let components = find_components(db, &db.crates());
    assert_eq!(components.len(), 1);
    assert_eq!(components[0].name, "PositionComponent");
}
