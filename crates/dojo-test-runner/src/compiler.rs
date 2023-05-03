use anyhow::{bail, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_test_runner::TestRunner;
use dojo_lang::compiler::collect_main_crate_ids;
use scarb::compiler::{CompilationUnit, Compiler};
use scarb::core::Workspace;

pub struct DojoTestCompiler;

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
            // TODO: Pass these in
            filter: "".to_string(),
            include_ignored: false,
            ignored: false,
            starknet: true,
        };

        runner.run()?;

        Ok(())
    }
}
