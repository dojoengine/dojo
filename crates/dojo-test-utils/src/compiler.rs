use std::env::{self, current_dir};

use anyhow::Result;
use assert_fs::TempDir;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::{FilesGroup, FilesGroupEx};
use cairo_lang_filesystem::ids::{CrateLongId, Directory};
use cairo_lang_project::{ProjectConfig, ProjectConfigContent};
use camino::{Utf8Path, Utf8PathBuf};
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::db::DojoRootDatabaseBuilderEx;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::{CompilationUnit, CompilerRepository};
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;
use tracing::trace;

pub fn build_test_config(path: &str) -> anyhow::Result<Config> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::new();

    let cache_dir = TempDir::new().unwrap();
    let config_dir = TempDir::new().unwrap();

    let path = Utf8PathBuf::from_path_buf(path.into()).unwrap();
    Config::builder(path.canonicalize_utf8().unwrap())
        .global_cache_dir_override(Some(Utf8Path::from_path(cache_dir.path()).unwrap()))
        .global_config_dir_override(Some(Utf8Path::from_path(config_dir.path()).unwrap()))
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .cairo_plugins(cairo_plugins.into())
        .build()
}

pub fn build_test_db(manifest_path: &str) -> anyhow::Result<RootDatabase> {
    let config = build_test_config(manifest_path).unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();
    let resolve = ops::resolve_workspace(&ws)?;
    let compilation_units = ops::generate_compilation_units(&resolve, &ws)?;

    let unit: CompilationUnit = compilation_units[0].clone();

    let mut db = RootDatabase::builder()
        .with_project_config(build_project_config(&unit)?)
        .with_dojo()
        .build()?;

    let mut current_path = current_dir().unwrap();
    current_path.push("../dojo-core/src");

    let crate_id = db.intern_crate(CrateLongId("dojo".into()));
    let root = Directory(current_path);
    db.set_crate_root(crate_id, Some(root));

    Ok(db)
}

fn build_project_config(unit: &CompilationUnit) -> Result<ProjectConfig> {
    let crate_roots = unit
        .components
        .iter()
        .filter(|component| !component.package.id.is_core())
        .map(|component| (component.cairo_package_name(), component.target.source_root().into()))
        .collect();

    let corelib = Some(Directory(unit.core_package_component().target.source_root().into()));

    let content = ProjectConfigContent { crate_roots };

    let project_config =
        ProjectConfig { base_path: unit.main_component().package.root().into(), corelib, content };

    trace!(?project_config);

    Ok(project_config)
}
