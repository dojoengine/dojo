use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_semantic::test_utils::setup_test_crate;
use pretty_assertions::assert_eq;

use crate::system::find_systems;
use crate::testing::build_test_db;

#[test]
fn test_system_resolving() {
    let db = &mut build_test_db().unwrap();
    let _crate_id = setup_test_crate(
        db,
        "
            mod NotAsystem {}

            #[system]
            mod Move {
                #[execute]
                fn move() {}
            }
        ",
    );

    let systems = find_systems(db, &db.crates());
    assert_eq!(systems.len(), 1);
    assert_eq!(systems[0].name, "MoveSystem");
}
