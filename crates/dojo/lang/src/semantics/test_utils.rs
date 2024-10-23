use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cairo_lang_defs::db::{ext_as_virtual_impl, DefsDatabase, DefsGroup};
use cairo_lang_defs::ids::{FunctionWithBodyId, ModuleId};
use cairo_lang_diagnostics::{Diagnostics, DiagnosticsBuilder};
use cairo_lang_filesystem::db::{
    init_dev_corelib, init_files_group, AsFilesGroupMut, CrateConfiguration, ExternalFiles,
    FilesDatabase, FilesGroup, FilesGroupEx,
};
use cairo_lang_filesystem::ids::{
    CrateId, CrateLongId, Directory, FileKind, FileLongId, VirtualFile,
};
use cairo_lang_parser::db::{ParserDatabase, ParserGroup};
use cairo_lang_semantic::db::{SemanticDatabase, SemanticGroup};
use cairo_lang_semantic::inline_macros::get_default_plugin_suite;
use cairo_lang_semantic::items::functions::GenericFunctionId;
use cairo_lang_semantic::{ConcreteFunctionWithBodyId, SemanticDiagnostic};
use cairo_lang_starknet::starknet_plugin_suite;
use cairo_lang_syntax::node::db::{SyntaxDatabase, SyntaxGroup};
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::{extract_matches, OptionFrom, Upcast};
use camino::Utf8PathBuf;
use once_cell::sync::Lazy;
use scarb::compiler::Profile;

use crate::compiler::test_utils::{build_test_config, corelib};
use crate::plugin::dojo_plugin_suite;

#[salsa::database(
    SemanticDatabase,
    DefsDatabase,
    ParserDatabase,
    SyntaxDatabase,
    FilesDatabase
)]
#[allow(missing_debug_implementations)]
pub struct DojoSemanticDatabase {
    storage: salsa::Storage<DojoSemanticDatabase>,
}
impl salsa::Database for DojoSemanticDatabase {}
impl ExternalFiles for DojoSemanticDatabase {
    fn ext_as_virtual(&self, external_id: salsa::InternId) -> VirtualFile {
        ext_as_virtual_impl(self.upcast(), external_id)
    }
}
impl salsa::ParallelDatabase for DojoSemanticDatabase {
    fn snapshot(&self) -> salsa::Snapshot<DojoSemanticDatabase> {
        salsa::Snapshot::new(DojoSemanticDatabase {
            storage: self.storage.snapshot(),
        })
    }
}

impl DojoSemanticDatabase {
    pub fn new_empty() -> Self {
        let mut db = DojoSemanticDatabase {
            storage: Default::default(),
        };
        init_files_group(&mut db);

        let mut suite = get_default_plugin_suite();
        suite.add(starknet_plugin_suite());
        suite.add(dojo_plugin_suite());

        db.set_macro_plugins(suite.plugins);
        db.set_inline_macro_plugins(suite.inline_macro_plugins.into());
        db.set_analyzer_plugins(suite.analyzer_plugins);

        let dojo_path = Utf8PathBuf::from_path_buf("../../crates/contracts/src".into()).unwrap();
        let dojo_path: PathBuf = dojo_path.canonicalize_utf8().unwrap().into();
        let dojo_scarb_manifest = dojo_path.parent().unwrap().join("Scarb.toml");
        let core_crate = db.intern_crate(CrateLongId::Real("dojo".into()));
        let core_root_dir = Directory::Real(dojo_path);

        // Use a config to detect the corelib.
        let config =
            build_test_config(dojo_scarb_manifest.to_str().unwrap(), Profile::DEV).unwrap();

        // Ensure the crate[0] is dojo, to enable parsing of the Scarb.toml.
        db.set_crate_config(
            core_crate,
            Some(CrateConfiguration::default_for_root(core_root_dir)),
        );

        init_dev_corelib(&mut db, corelib(&config));
        db
    }
    /// Snapshots the db for read only.
    pub fn snapshot(&self) -> DojoSemanticDatabase {
        DojoSemanticDatabase {
            storage: self.storage.snapshot(),
        }
    }
}

pub static SHARED_DB: Lazy<Mutex<DojoSemanticDatabase>> =
    Lazy::new(|| Mutex::new(DojoSemanticDatabase::new_empty()));

impl Default for DojoSemanticDatabase {
    fn default() -> Self {
        SHARED_DB.lock().unwrap().snapshot()
    }
}
impl AsFilesGroupMut for DojoSemanticDatabase {
    fn as_files_group_mut(&mut self) -> &mut (dyn FilesGroup + 'static) {
        self
    }
}
impl Upcast<dyn FilesGroup> for DojoSemanticDatabase {
    fn upcast(&self) -> &(dyn FilesGroup + 'static) {
        self
    }
}
impl Upcast<dyn SyntaxGroup> for DojoSemanticDatabase {
    fn upcast(&self) -> &(dyn SyntaxGroup + 'static) {
        self
    }
}
impl Upcast<dyn DefsGroup> for DojoSemanticDatabase {
    fn upcast(&self) -> &(dyn DefsGroup + 'static) {
        self
    }
}
impl Upcast<dyn SemanticGroup> for DojoSemanticDatabase {
    fn upcast(&self) -> &(dyn SemanticGroup + 'static) {
        self
    }
}
impl Upcast<dyn ParserGroup> for DojoSemanticDatabase {
    fn upcast(&self) -> &(dyn ParserGroup + 'static) {
        self
    }
}

#[derive(Debug)]
pub struct WithStringDiagnostics<T> {
    value: T,
    diagnostics: String,
}
impl<T> WithStringDiagnostics<T> {
    /// Verifies that there are no diagnostics (fails otherwise), and returns the inner value.
    pub fn unwrap(self) -> T {
        assert!(
            self.diagnostics.is_empty(),
            "Unexpected diagnostics:\n{}",
            self.diagnostics
        );
        self.value
    }

    /// Returns the inner value and the diagnostics (as a string).
    pub fn split(self) -> (T, String) {
        (self.value, self.diagnostics)
    }

    /// Returns the diagnostics (as a string).
    pub fn get_diagnostics(self) -> String {
        self.diagnostics
    }
}

/// Helper struct for the return value of [setup_test_module].
#[derive(Debug)]
pub struct TestModule {
    pub crate_id: CrateId,
    pub module_id: ModuleId,
}

/// Sets up a crate with given content, and returns its crate id.
pub fn setup_test_crate(db: &dyn SemanticGroup, content: &str) -> CrateId {
    let file_id = db.intern_file(FileLongId::Virtual(VirtualFile {
        parent: None,
        name: "lib.cairo".into(),
        content: content.into(),
        code_mappings: Arc::new([]),
        kind: FileKind::Module,
    }));

    db.intern_crate(CrateLongId::Virtual {
        name: "test".into(),
        config: CrateConfiguration::default_for_root(Directory::Virtual {
            files: BTreeMap::from([("lib.cairo".into(), file_id)]),
            dirs: Default::default(),
        }),
    })
}

/// Sets up a module with given content, and returns its module id.
pub fn setup_test_module(
    db: &(dyn SemanticGroup + 'static),
    content: &str,
) -> WithStringDiagnostics<TestModule> {
    let crate_id = setup_test_crate(db, content);
    let module_id = ModuleId::CrateRoot(crate_id);
    let file_id = db.module_main_file(module_id).unwrap();

    let syntax_diagnostics = db
        .file_syntax_diagnostics(file_id)
        .format(Upcast::upcast(db));
    let semantic_diagnostics = db
        .module_semantic_diagnostics(module_id)
        .unwrap()
        .format(db);

    WithStringDiagnostics {
        value: TestModule {
            crate_id,
            module_id,
        },
        diagnostics: format!("{syntax_diagnostics}{semantic_diagnostics}"),
    }
}

/// Helper struct for the return value of [setup_test_function].
#[derive(Debug)]
pub struct TestFunction {
    pub module_id: ModuleId,
    pub function_id: FunctionWithBodyId,
    pub concrete_function_id: ConcreteFunctionWithBodyId,
    pub signature: cairo_lang_semantic::Signature,
    pub body: cairo_lang_semantic::ExprId,
}

/// Returns the semantic model of a given function.
/// function_name - name of the function.
/// module_code - extra setup code in the module context.
pub fn setup_test_function(
    db: &(dyn SemanticGroup + 'static),
    function_code: &str,
    function_name: &str,
    module_code: &str,
) -> WithStringDiagnostics<TestFunction> {
    let content = if module_code.is_empty() {
        function_code.to_string()
    } else {
        format!("{module_code}\n{function_code}")
    };
    let (test_module, diagnostics) = setup_test_module(db, &content).split();
    let generic_function_id = db
        .module_item_by_name(test_module.module_id, function_name.into())
        .expect("Failed to load module")
        .and_then(GenericFunctionId::option_from)
        .unwrap_or_else(|| panic!("Function '{function_name}' was not found."));
    let free_function_id = extract_matches!(generic_function_id, GenericFunctionId::Free);
    let function_id = FunctionWithBodyId::Free(free_function_id);
    WithStringDiagnostics {
        value: TestFunction {
            module_id: test_module.module_id,
            function_id,
            concrete_function_id: ConcreteFunctionWithBodyId::from_no_generics_free(
                db,
                free_function_id,
            )
            .unwrap(),
            signature: db.function_with_body_signature(function_id).unwrap(),
            body: db.function_body_expr(function_id).unwrap(),
        },
        diagnostics,
    }
}

/// Helper struct for the return value of [setup_test_expr] and [setup_test_block].
#[derive(Debug)]
pub struct TestExpr {
    pub module_id: ModuleId,
    pub function_id: FunctionWithBodyId,
    pub signature: cairo_lang_semantic::Signature,
    pub body: cairo_lang_semantic::ExprId,
    pub expr_id: cairo_lang_semantic::ExprId,
}

/// Returns the semantic model of a given expression.
/// module_code - extra setup code in the module context.
/// function_body - extra setup code in the function context.
pub fn setup_test_expr(
    db: &(dyn SemanticGroup + 'static),
    expr_code: &str,
    module_code: &str,
    function_body: &str,
) -> WithStringDiagnostics<TestExpr> {
    let function_code = format!("fn test_func() {{ {function_body} {{\n{expr_code}\n}}; }}");
    let (test_function, diagnostics) =
        setup_test_function(db, &function_code, "test_func", module_code).split();
    let cairo_lang_semantic::ExprBlock { statements, .. } = extract_matches!(
        db.expr_semantic(test_function.function_id, test_function.body),
        cairo_lang_semantic::Expr::Block
    );
    let statement_expr = extract_matches!(
        db.statement_semantic(test_function.function_id, *statements.last().unwrap()),
        cairo_lang_semantic::Statement::Expr
    );
    let cairo_lang_semantic::ExprBlock {
        statements, tail, ..
    } = extract_matches!(
        db.expr_semantic(test_function.function_id, statement_expr.expr),
        cairo_lang_semantic::Expr::Block
    );
    assert!(
        statements.is_empty(),
        "expr_code is not a valid expression. Consider using setup_test_block()."
    );
    WithStringDiagnostics {
        value: TestExpr {
            module_id: test_function.module_id,
            function_id: test_function.function_id,
            signature: test_function.signature,
            body: test_function.body,
            expr_id: tail.unwrap(),
        },
        diagnostics,
    }
}

/// Returns the semantic model of a given block expression.
/// module_code - extra setup code in the module context.
/// function_body - extra setup code in the function context.
pub fn setup_test_block(
    db: &(dyn SemanticGroup + 'static),
    expr_code: &str,
    module_code: &str,
    function_body: &str,
) -> WithStringDiagnostics<TestExpr> {
    setup_test_expr(
        db,
        &format!("{{ \n{expr_code}\n }}"),
        module_code,
        function_body,
    )
}

pub fn test_expr_diagnostics(
    inputs: &OrderedHashMap<String, String>,
    _args: &OrderedHashMap<String, String>,
) -> Result<OrderedHashMap<String, String>, String> {
    let db = &DojoSemanticDatabase::default();
    Ok(OrderedHashMap::from([(
        "expected_diagnostics".into(),
        setup_test_expr(
            db,
            inputs["expr_code"].as_str(),
            inputs["module_code"].as_str(),
            inputs["function_body"].as_str(),
        )
        .get_diagnostics(),
    )]))
}

pub fn test_function_diagnostics(
    inputs: &OrderedHashMap<String, String>,
    _args: &OrderedHashMap<String, String>,
) -> Result<OrderedHashMap<String, String>, String> {
    let db = &DojoSemanticDatabase::default();
    Ok(OrderedHashMap::from([(
        "expected_diagnostics".into(),
        setup_test_function(
            db,
            inputs["function"].as_str(),
            inputs["function_name"].as_str(),
            inputs["module_code"].as_str(),
        )
        .get_diagnostics(),
    )]))
}

/// Gets the diagnostics for all the modules (including nested) in the given crate.
pub fn get_crate_semantic_diagnostics(
    db: &dyn SemanticGroup,
    crate_id: CrateId,
) -> Diagnostics<SemanticDiagnostic> {
    let submodules = db.crate_modules(crate_id);
    let mut diagnostics = DiagnosticsBuilder::default();
    for submodule_id in submodules.iter() {
        diagnostics.extend(db.module_semantic_diagnostics(*submodule_id).unwrap());
    }
    diagnostics.build()
}
