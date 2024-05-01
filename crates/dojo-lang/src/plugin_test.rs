use std::sync::Arc;

use cairo_lang_defs::db::{DefsDatabase, DefsGroup};
use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
use cairo_lang_defs::plugin::MacroPlugin;
use cairo_lang_filesystem::cfg::CfgSet;
use cairo_lang_filesystem::db::{
    init_files_group, AsFilesGroupMut, CrateConfiguration, FilesDatabase, FilesGroup, FilesGroupEx,
};
use cairo_lang_filesystem::ids::{CrateLongId, Directory, FileLongId};
use cairo_lang_parser::db::ParserDatabase;
use cairo_lang_plugins::get_base_plugins;
use cairo_lang_plugins::test_utils::expand_module_text;
use cairo_lang_syntax::attribute::structured::AttributeListStructurize;
use cairo_lang_syntax::node::db::{SyntaxDatabase, SyntaxGroup};
use cairo_lang_test_utils::parse_test_file::TestRunnerResult;
use cairo_lang_test_utils::verify_diagnostics_expectation;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::Upcast;

use super::BuiltinDojoPlugin;

cairo_lang_test_utils::test_file_test!(
    expand_plugin,
    "src/plugin_test_data",
    {
        model: "model",
        print: "print",
        introspect: "introspect",
        system: "system",
    },
    test_expand_plugin
);

pub fn test_expand_plugin(
    inputs: &OrderedHashMap<String, String>,
    args: &OrderedHashMap<String, String>,
) -> TestRunnerResult {
    test_expand_plugin_inner(inputs, args, &[Arc::new(BuiltinDojoPlugin)])
}

#[salsa::database(DefsDatabase, ParserDatabase, SyntaxDatabase, FilesDatabase)]
pub struct DatabaseForTesting {
    storage: salsa::Storage<DatabaseForTesting>,
}
impl salsa::Database for DatabaseForTesting {}
impl Default for DatabaseForTesting {
    fn default() -> Self {
        let mut res = Self { storage: Default::default() };
        init_files_group(&mut res);
        res.set_macro_plugins(get_base_plugins());
        res
    }
}
impl AsFilesGroupMut for DatabaseForTesting {
    fn as_files_group_mut(&mut self) -> &mut (dyn FilesGroup + 'static) {
        self
    }
}
impl Upcast<dyn DefsGroup> for DatabaseForTesting {
    fn upcast(&self) -> &(dyn DefsGroup + 'static) {
        self
    }
}
impl Upcast<dyn FilesGroup> for DatabaseForTesting {
    fn upcast(&self) -> &(dyn FilesGroup + 'static) {
        self
    }
}
impl Upcast<dyn SyntaxGroup> for DatabaseForTesting {
    fn upcast(&self) -> &(dyn SyntaxGroup + 'static) {
        self
    }
}

/// Tests expansion of given code, with the default plugins plus the given extra plugins.
pub fn test_expand_plugin_inner(
    inputs: &OrderedHashMap<String, String>,
    args: &OrderedHashMap<String, String>,
    extra_plugins: &[Arc<dyn MacroPlugin>],
) -> TestRunnerResult {
    let db = &mut DatabaseForTesting::default();
    let mut plugins = db.macro_plugins();
    plugins.extend_from_slice(extra_plugins);
    db.set_macro_plugins(plugins);

    let cfg_set: Option<CfgSet> =
        inputs.get("cfg").map(|s| serde_json::from_str(s.as_str()).unwrap());
    if let Some(cfg_set) = cfg_set {
        db.set_cfg_set(Arc::new(cfg_set));
    }

    let cairo_code = &inputs["cairo_code"];

    let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
    let root = Directory::Real("test_src".into());

    db.set_crate_config(crate_id, Some(CrateConfiguration::default_for_root(root)));

    // Main module file.
    let file_id = db.intern_file(FileLongId::OnDisk("test_src/lib.cairo".into()));
    db.as_files_group_mut()
        .override_file_content(file_id, Some(Arc::new(format!("{cairo_code}\n"))));

    let mut diagnostic_items = vec![];
    let expanded_module =
        expand_module_text(db, ModuleId::CrateRoot(crate_id), &mut diagnostic_items);
    let joined_diagnostics = diagnostic_items.join("\n");
    let error = verify_diagnostics_expectation(args, &joined_diagnostics);

    TestRunnerResult {
        outputs: OrderedHashMap::from([
            ("expanded_cairo_code".into(), expanded_module),
            ("expected_diagnostics".into(), joined_diagnostics),
        ]),
        error,
    }
}

#[test]
pub fn test_extract_allow_ref_self_ok() {
    let cairo_code = r#"
    #[dojo::contract(allow_ref_self)]
    mod dojo_contract {}
    "#;

    let db = init_db_for_code(cairo_code);
    let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
    let crate_root = ModuleId::CrateRoot(crate_id);
    assert!(verify_extract_allow_ref_self(&db, crate_root));
}

#[test]
pub fn test_extract_allow_ref_self_ok_no_attr() {
    let cairo_code = r#"
    #[dojo::contract()]
    mod dojo_contract {}
    "#;

    let db = init_db_for_code(cairo_code);
    let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
    let crate_root = ModuleId::CrateRoot(crate_id);
    assert!(!verify_extract_allow_ref_self(&db, crate_root));
}

#[test]
pub fn test_extract_allow_ref_self_none() {
    let cairo_code = r#"
    mod other_contract {}
    "#;

    let db = init_db_for_code(cairo_code);
    let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
    let crate_root = ModuleId::CrateRoot(crate_id);
    assert!(!verify_extract_allow_ref_self(&db, crate_root));
}

pub fn init_db_for_code(cairo_code: &str) -> DatabaseForTesting {
    let mut db = DatabaseForTesting::default();

    let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
    let root = Directory::Real("test_src".into());

    db.set_crate_config(crate_id, Some(CrateConfiguration::default_for_root(root)));

    // Main module file.
    let file_id = db.intern_file(FileLongId::OnDisk("test_src/lib.cairo".into()));
    db.as_files_group_mut()
        .override_file_content(file_id, Some(Arc::new(format!("{cairo_code}\n"))));

    db
}

pub fn verify_extract_allow_ref_self(db: &dyn DefsGroup, module_id: ModuleId) -> bool {
    let syntax_db = db.upcast();

    for item_id in db.module_items(module_id).unwrap().iter() {
        if let ModuleItemId::Submodule(item) = item_id {
            let module_ast = item.stable_ptr(db).lookup(syntax_db);
            let attrs = module_ast.attributes(syntax_db).structurize(syntax_db);
            let dojo_contract_attr =
                attrs.iter().find(|attr| attr.id.as_str() == crate::plugin::DOJO_CONTRACT_ATTR);
            let do_allow_ref_self =
                crate::contract::extract_allow_ref_self(dojo_contract_attr, syntax_db)
                    .unwrap_or_default();

            if do_allow_ref_self {
                return true;
            }
        }
    }

    false
}
