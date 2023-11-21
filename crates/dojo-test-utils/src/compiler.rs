use std::env;
use std::path::PathBuf;

use assert_fs::TempDir;
use camino::{Utf8Path, Utf8PathBuf};
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb_ui::Verbosity;

pub fn build_test_config(path: &str) -> anyhow::Result<Config> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::default();

    let cache_dir = TempDir::new().unwrap();
    let config_dir = TempDir::new().unwrap();
    let target_dir = TempDir::new().unwrap();

    let path = Utf8PathBuf::from_path_buf(path.into()).unwrap();
    Config::builder(path.canonicalize_utf8().unwrap())
        .global_cache_dir_override(Some(Utf8Path::from_path(cache_dir.path()).unwrap()))
        .global_config_dir_override(Some(Utf8Path::from_path(config_dir.path()).unwrap()))
        .target_dir_override(Some(Utf8Path::from_path(target_dir.path()).unwrap().to_path_buf()))
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .cairo_plugins(cairo_plugins.into())
        .build()
}

pub fn corelib() -> PathBuf {
    let config = build_test_config("./src/manifest_test_data/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();
    let resolve = ops::resolve_workspace(&ws).unwrap();
    let compilation_units = ops::generate_compilation_units(&resolve, &ws).unwrap();
    compilation_units[0].core_package_component().target.source_root().into()
}
