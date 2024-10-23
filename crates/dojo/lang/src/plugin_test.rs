use std::path::PathBuf;
use std::sync::Arc;

use cairo_lang_defs::db::{ext_as_virtual_impl, DefsDatabase, DefsGroup};
use cairo_lang_defs::ids::ModuleId;
use cairo_lang_defs::plugin::MacroPlugin;
use cairo_lang_filesystem::cfg::{Cfg, CfgSet};
use cairo_lang_filesystem::db::{
    init_files_group, AsFilesGroupMut, CrateConfiguration, ExternalFiles, FilesDatabase,
    FilesGroup, FilesGroupEx,
};
use cairo_lang_filesystem::ids::{CrateLongId, Directory, VirtualFile};
use cairo_lang_parser::db::ParserDatabase;
use cairo_lang_plugins::get_base_plugins;
use cairo_lang_plugins::test_utils::expand_module_text;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use cairo_lang_syntax::node::db::{SyntaxDatabase, SyntaxGroup};
use cairo_lang_test_utils::parse_test_file::TestRunnerResult;
use cairo_lang_test_utils::verify_diagnostics_expectation;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::Upcast;
use smol_str::SmolStr;

use super::BuiltinDojoPlugin;
use crate::namespace_config::DEFAULT_NAMESPACE_CFG_KEY;

#[salsa::database(DefsDatabase, ParserDatabase, SyntaxDatabase, FilesDatabase)]
#[allow(missing_debug_implementations)]
pub struct DatabaseForTesting {
    storage: salsa::Storage<DatabaseForTesting>,
}
impl salsa::Database for DatabaseForTesting {}
impl ExternalFiles for DatabaseForTesting {
    fn ext_as_virtual(&self, external_id: salsa::InternId) -> VirtualFile {
        ext_as_virtual_impl(self.upcast(), external_id)
    }
}
impl Default for DatabaseForTesting {
    fn default() -> Self {
        let mut res = Self {
            storage: Default::default(),
        };
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

cairo_lang_test_utils::test_file_test!(
    expand_plugin,
    "src/plugin/plugin_test_data",
    {
        model: "model",
        event: "event",
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
    test_expand_plugin_inner(
        inputs,
        args,
        &[
            Arc::new(BuiltinDojoPlugin),
            Arc::new(StarkNetPlugin::default()),
        ],
    )
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

    // if no configuration is provided, be sure there is at least a default namespace,
    // so all the tests have a correct configuration.
    let cfg_set: CfgSet = match inputs.get("cfg") {
        Some(cfg) => serde_json::from_str(cfg.as_str()).unwrap(),
        None => {
            let mut cfg_set = CfgSet::new();
            cfg_set.insert(Cfg::kv(
                DEFAULT_NAMESPACE_CFG_KEY,
                SmolStr::from("dojo_test"),
            ));
            cfg_set
        }
    };

    db.set_cfg_set(Arc::new(cfg_set));

    let test_id = &inputs["test_id"];
    let cairo_code = &inputs["cairo_code"];

    // The path as to remain the same, because diagnostics contains the path
    // of the file. Which can cause error when checked without CAIRO_FIX=1.
    let tmp_dir = PathBuf::from(format!("/tmp/plugin_test/{}", test_id));
    let _ = std::fs::create_dir_all(&tmp_dir);
    let tmp_path = tmp_dir.as_path();

    // Create Scarb.toml file
    let scarb_toml_path = tmp_path.join("Scarb.toml");
    std::fs::write(
        scarb_toml_path,
        r#"
[package]
cairo-version = "=2.6.4"
edition = "2024_07"
name = "test_package"
version = "0.7.3"

[cairo]
sierra-replace-ids = true

[[target.dojo]]

[tool.dojo.world]
namespace = { default = "test_package" }
seed = "test_package"
"#,
    )
    .expect("Failed to write Scarb.toml");

    // Create src directory
    let src_dir = tmp_path.join("src");
    let _ = std::fs::create_dir(&src_dir);

    // Create lib.cairo file
    let lib_cairo_path = src_dir.join("lib.cairo");
    std::fs::write(lib_cairo_path, cairo_code).expect("Failed to write lib.cairo");

    let crate_id = db.intern_crate(CrateLongId::Real("test".into()));
    let root = Directory::Real(src_dir.to_path_buf());

    db.set_crate_config(crate_id, Some(CrateConfiguration::default_for_root(root)));

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
