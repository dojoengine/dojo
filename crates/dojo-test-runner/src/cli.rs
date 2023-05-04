//! Compiles and runs a Dojo project.

use std::env::{self, current_dir};

use camino::Utf8PathBuf;
use clap::Parser;
use compiler::DojoTestCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;

mod compiler;

/// Command line args parser.
/// Exits with 0/1 if the input is formatted correctly/incorrectly.
#[derive(Parser, Debug)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// The path to compile and run its tests.
    path: Utf8PathBuf,
    /// The filter for the tests, running only tests containing the filter string.
    #[arg(short, long, default_value_t = String::default())]
    filter: String,
    /// Should we run ignored tests as well.
    #[arg(long, default_value_t = false)]
    include_ignored: bool,
    /// Should we run only the ignored tests.
    #[arg(long, default_value_t = false)]
    ignored: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let source_dir = if args.path.is_absolute() {
        args.path
    } else {
        let mut current_path = current_dir().unwrap();
        current_path.push(args.path);
        Utf8PathBuf::from_path_buf(current_path).unwrap()
    };

    let mut compilers = CompilerRepository::std();
    compilers.add(Box::new(DojoTestCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::new()?;

    let manifest_path = source_dir.join("Scarb.toml");
    let config = Config::builder(manifest_path)
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .cairo_plugins(cairo_plugins.into())
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });

    ops::compile(&ws)
}
