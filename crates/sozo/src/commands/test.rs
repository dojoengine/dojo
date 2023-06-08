//! Compiles and runs tests for a Dojo project.

use std::env::{self, current_dir};

use anyhow::{bail, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_test_runner::TestRunner;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_lang::compiler::collect_main_crate_ids;
use dojo_lang::plugin::CairoPluginRepository;
use scarb::compiler::{CompilationUnit, Compiler, CompilerRepository};
use scarb::core::{Config, Workspace};
use scarb::ops;
use scarb::ui::Verbosity;

#[derive(Args)]
pub struct TestArgs {
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

pub fn run(args: TestArgs) -> anyhow::Result<()> {
    let source_dir = if args.path.is_absolute() {
        args.path.clone()
    } else {
        let mut current_path = current_dir().unwrap();
        current_path.push(args.path.clone());
        Utf8PathBuf::from_path_buf(current_path).unwrap()
    };

    let mut compilers = CompilerRepository::std();
    compilers.add(Box::new(DojoTestCompiler { args })).unwrap();

    let cairo_plugins = CairoPluginRepository::new();

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

pub struct DojoTestCompiler {
    args: TestArgs,
}

impl Compiler for DojoTestCompiler {
    fn target_kind(&self) -> &str {
        "dojo"
    }

    fn compile(
        &self,
        unit: CompilationUnit,
        db: &mut RootDatabase,
        _: &Workspace<'_>,
    ) -> Result<()> {
        let main_crate_ids = collect_main_crate_ids(&unit, db);

        if DiagnosticsReporter::stderr().check(db) {
            bail!("failed to compile");
        }

        let runner = TestRunner {
            db: db.snapshot(),
            main_crate_ids,
            filter: self.args.filter.clone(),
            include_ignored: self.args.include_ignored,
            ignored: self.args.ignored,
            starknet: true,
        };

        runner.run()?;

        Ok(())
    }
}
