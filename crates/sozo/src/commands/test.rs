//! Compiles and runs tests for a Dojo project.

use anyhow::{bail, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_test_runner::TestRunner;
use clap::Args;
use dojo_lang::compiler::collect_main_crate_ids;
use scarb::compiler::{CompilationUnit, Compiler};
use scarb::core::{Config, Workspace};
use scarb::ops;

#[derive(Args, Clone)]
pub struct TestArgs {
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

impl TestArgs {
    pub fn run(self, config: &Config) -> anyhow::Result<()> {
        let ws = ops::read_workspace(config.manifest_path(), config).unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(1);
        });

        ops::compile(&ws)
    }
}

pub struct DojoTestCompiler {
    args: TestArgs,
}

impl DojoTestCompiler {
    pub fn new(args: TestArgs) -> Self {
        Self { args }
    }
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
