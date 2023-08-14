//! Compiles and runs tests for a Dojo project.

use std::sync::Arc;

use anyhow::{bail, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::cfg::{Cfg, CfgSet};
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use cairo_lang_test_runner::plugin::TestPlugin;
use cairo_lang_test_runner::TestRunner;
use clap::Args;
use dojo_lang::compiler::{collect_core_crate_ids, collect_external_crate_ids, Props};
use dojo_lang::plugin::DojoPlugin;
use scarb::compiler::helpers::collect_main_crate_ids;
use scarb::compiler::CompilationUnit;
use scarb::core::Config;
use scarb::ops;
use tracing::trace;

/// Execute all unit tests of a local package.
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

        let resolve = ops::resolve_workspace(&ws)?;
        let compilation_units = ops::generate_compilation_units(&resolve, &ws)?;

        for unit in compilation_units {
            let props: Props = unit.target().props()?;
            let db = build_root_database(&unit)?;

            let mut main_crate_ids = collect_main_crate_ids(&unit, &db);

            if unit.main_package_id.name.to_string() != "dojo" {
                let core_crate_ids = collect_core_crate_ids(&db);
                main_crate_ids.extend(core_crate_ids);
            }

            if let Some(external_contracts) = props.build_external_contracts {
                main_crate_ids.extend(collect_external_crate_ids(&db, external_contracts));
            }

            if DiagnosticsReporter::stderr().check(&db) {
                bail!("failed to compile");
            }

            let runner = TestRunner {
                db,
                main_crate_ids,
                filter: self.filter.clone(),
                include_ignored: self.include_ignored,
                ignored: self.ignored,
                starknet: true,
            };
            runner.run()?;

            println!();
        }

        Ok(())
    }
}

pub(crate) fn build_root_database(unit: &CompilationUnit) -> Result<RootDatabase> {
    let mut b = RootDatabase::builder();
    b.with_project_config(build_project_config(unit)?);
    b.with_cfg(CfgSet::from_iter([Cfg::name("test")]));

    b.with_semantic_plugin(Arc::new(TestPlugin::default()));
    b.with_semantic_plugin(Arc::new(DojoPlugin));
    b.with_semantic_plugin(Arc::new(StarkNetPlugin::default()));

    b.build()
}

fn build_project_config(unit: &CompilationUnit) -> Result<ProjectConfig> {
    let crate_roots = unit
        .components
        .iter()
        .filter(|component| !component.package.id.is_core())
        .map(|component| (component.cairo_package_name(), component.target.source_root().into()))
        .collect();

    let corelib = Some(Directory(unit.core_package_component().target.source_root().into()));

    let content = ProjectConfigContent { crate_roots };

    let project_config =
        ProjectConfig { base_path: unit.main_component().package.root().into(), corelib, content };

    trace!(?project_config);

    Ok(project_config)
}
