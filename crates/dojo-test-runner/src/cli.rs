//! Compiles and runs a Dojo project.

use std::env::{self, current_dir};

use anyhow::bail;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_test_runner::TestRunner;
use camino::Utf8PathBuf;
use clap::Parser;
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::db::DojoRootDatabaseBuilderEx;
use scarb::compiler::helpers::{build_project_config, collect_main_crate_ids};
use scarb::compiler::CompilerRepository;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;

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

    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let manifest_path = source_dir.join("Scarb.toml");
    let config = Config::builder(manifest_path)
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });

    let resolve = ops::resolve_workspace(&ws)?;
    let compilation_units = ops::generate_compilation_units(&resolve, &ws)?;

    let unit = compilation_units[0].clone();

    let db = &mut RootDatabase::builder()
        .with_project_config(build_project_config(&unit)?)
        .with_dojo()
        .build()?;

    let main_crate_ids = collect_main_crate_ids(&unit, db);

    if DiagnosticsReporter::stderr().check(db) {
        bail!("failed to compile");
    }

    let runner = TestRunner {
        db: db.snapshot(),
        main_crate_ids,
        filter: args.filter,
        include_ignored: args.include_ignored,
        ignored: args.ignored,
        starknet: true,
    };

    runner.run()?;

    Ok(())
}
