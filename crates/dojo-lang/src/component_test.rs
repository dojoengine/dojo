use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_semantic::test_utils::setup_test_crate;
use dojo_project::ProjectConfig;
use indoc::indoc;
use pretty_assertions::assert_eq;

use crate::component::find_components;
use crate::db::DojoRootDatabaseBuilderEx;

#[test]
fn test_component_resolving() {
    let db =
        &mut RootDatabase::builder().with_dojo_config(ProjectConfig::default()).build().unwrap();
    let _crate_id = setup_test_crate(
        db,
        indoc! {"
            mod NotAComponent {}

            #[component]
            mod Position {
                struct State {
                    x: u64,
                    y: u64,
                }
            }
        "},
    );

    let components = find_components(db, &db.crates());
    assert_eq!(components.len(), 1);
    assert_eq!(components[0].name, "Position");
}
