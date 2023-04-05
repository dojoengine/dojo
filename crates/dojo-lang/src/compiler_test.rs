use std::env;

use camino::Utf8PathBuf;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;

use super::DojoCompiler;

#[test]
fn test_compiler() {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let path = Utf8PathBuf::from_path_buf("src/cairo_level_tests/Scarb.toml".into()).unwrap();

    let config = Config::builder(path.canonicalize_utf8().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();

    ops::compile(&ws).unwrap_or_else(|op| panic!("Error compiling: {:?}", op))
}
