use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_semantic::test_utils::setup_test_crate;
use dojo_project::ProjectConfig;
use indoc::indoc;
use pretty_assertions::assert_eq;

use crate::db::DojoRootDatabaseBuilderEx;
use crate::system::find_systems;

#[test]
fn test_system_resolving() {
    let db =
        &mut RootDatabase::builder().with_dojo_config(ProjectConfig::default()).build().unwrap();

    let systems = find_systems(db, &db.crates());
    assert_eq!(systems.len(), 1);
    assert_eq!(systems[0].name, "MoveSystem");
}
