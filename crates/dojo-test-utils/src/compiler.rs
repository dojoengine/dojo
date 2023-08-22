use std::env::{self};






use camino::Utf8PathBuf;
use dojo_lang::compiler::DojoCompiler;

use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::{CompilerRepository};
use scarb::core::Config;

use scarb::ui::Verbosity;

pub fn build_test_config(path: &str) -> anyhow::Result<Config> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::new();

    let path = Utf8PathBuf::from_path_buf(path.into()).unwrap();

    Config::builder(path.canonicalize_utf8().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .cairo_plugins(cairo_plugins.into())
        .compilers(compilers)
        .offline(true)
        .build()
}
