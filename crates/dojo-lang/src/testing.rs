use std::env;

use anyhow::Result;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_project::{ProjectConfig, ProjectConfigContent};
use camino::Utf8PathBuf;
use scarb::compiler::{CompilationUnit, CompilerRepository};
use scarb::core::{Config, Workspace};
use scarb::ops;
use scarb::ui::Verbosity;
use tracing::trace;

use crate::compiler::DojoCompiler;
use crate::db::DojoRootDatabaseBuilderEx;
use crate::plugin::CairoPluginRepository;

pub fn build_test_config() -> anyhow::Result<Config> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::new()?;

    let path = Utf8PathBuf::from_path_buf("src/cairo_level_tests/Scarb.toml".into()).unwrap();
    Config::builder(path.canonicalize_utf8().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .cairo_plugins(cairo_plugins.into())
        .build()
}

pub fn build_test_db(ws: &Workspace<'_>) -> anyhow::Result<RootDatabase> {
    let resolve = ops::resolve_workspace(ws)?;
    let compilation_units = ops::generate_compilation_units(&resolve, ws)?;

    let unit = compilation_units[0].clone();

    let db = RootDatabase::builder()
        .with_project_config(build_project_config(&unit)?)
        .with_dojo()
        .build()?;

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
