use std::env::{self};

use assert_fs::TempDir;
use camino::{Utf8Path, Utf8PathBuf};
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ui::Verbosity;

pub fn build_test_config(path: &str) -> anyhow::Result<Config> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::new();

    let path = Utf8PathBuf::from_path_buf(path.into()).unwrap();

    let cache_dir = TempDir::new().unwrap();
    let config_dir = TempDir::new().unwrap();

    Config::builder(path.canonicalize_utf8().unwrap())
        .global_cache_dir_override(Some(Utf8Path::from_path(cache_dir.path()).unwrap()))
        .global_config_dir_override(Some(Utf8Path::from_path(config_dir.path()).unwrap()))
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .cairo_plugins(cairo_plugins.into())
        .build()
}
