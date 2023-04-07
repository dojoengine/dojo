use std::env;

use cairo_lang_compiler::db::RootDatabase;
use camino::Utf8PathBuf;
use scarb::compiler::helpers::build_project_config;
use scarb::compiler::CompilerRepository;
use scarb::core::{Config, Workspace};
use scarb::ops;
use scarb::ui::Verbosity;

use crate::compiler::DojoCompiler;
use crate::db::DojoRootDatabaseBuilderEx;

pub fn build_test_config() -> anyhow::Result<Config> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let path = Utf8PathBuf::from_path_buf("src/cairo_level_tests/Scarb.toml".into()).unwrap();
    Config::builder(path.canonicalize_utf8().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
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
