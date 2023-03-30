use std::env;
use std::str::FromStr;

use cairo_lang_compiler::db::RootDatabase;
use camino::Utf8PathBuf;
use scarb::compiler::helpers::build_project_config;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;

use crate::compiler::DojoCompiler;
use crate::db::DojoRootDatabaseBuilderEx;

pub fn build_test_db() -> anyhow::Result<RootDatabase> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let dojo_dir = env::var("DOJOLIB_DIR")
        .unwrap_or_else(|e| panic!("Problem getting the dojolib path: {e:?}"));
    let dojo_path = Utf8PathBuf::from_str(dojo_dir.as_str())
        .unwrap_or_else(|e| panic!("Problem parsing the dojolib path: {e:?}"));
    let manifest_path = dojo_path.join("Scarb.toml");

    let config = Config::builder(manifest_path)
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {}", err);
        std::process::exit(1);
    });

    let resolve = ops::resolve_workspace(&ws)?;
    let compilation_units = ops::generate_compilation_units(&resolve, &ws)?;

    let unit = compilation_units[0].clone();

    let db = RootDatabase::builder()
        .with_project_config(build_project_config(&unit)?)
        .with_dojo()
        .build()?;

    Ok(db)
}
