//! Compiles and runs tests for a Dojo project.
use anyhow::{bail, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::cfg::{Cfg, CfgSet};
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_starknet::starknet_plugin_suite;
use cairo_lang_test_plugin::test_plugin_suite;
use cairo_lang_test_runner::{CompiledTestRunner, RunProfilerConfig, TestCompiler, TestRunConfig};
use clap::Args;
use dojo_lang::compiler::{collect_core_crate_ids, collect_external_crate_ids, Props};
use dojo_lang::plugin::dojo_plugin_suite;
use dojo_lang::scarb_internal::crates_config_for_compilation_unit;
use scarb::compiler::helpers::collect_main_crate_ids;
use scarb::compiler::{CairoCompilationUnit, CompilationUnit, CompilationUnitAttributes};
use scarb::core::Config;
use scarb::ops;
use scarb_ui::args::FeaturesSpec;
use tracing::trace;

pub(crate) const LOG_TARGET: &str = "sozo::cli::commands::test";

#[derive(Debug, Clone, PartialEq, clap::ValueEnum)]
pub enum ProfilerMode {
    None,
    Cairo,
    Sierra,
}

impl From<ProfilerMode> for RunProfilerConfig {
    fn from(mode: ProfilerMode) -> Self {
        match mode {
            ProfilerMode::None => RunProfilerConfig::None,
            ProfilerMode::Cairo => RunProfilerConfig::Cairo,
            ProfilerMode::Sierra => RunProfilerConfig::Sierra,
        }
    }
}

/// Execute all unit tests of a local package.
#[derive(Debug, Args)]
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
    /// Should we run the profiler and with what mode.
    #[arg(long, default_value = "none")]
    profiler_mode: ProfilerMode,
    /// Should we run the tests with gas enabled.
    #[arg(long, default_value_t = true)]
    gas_enabled: bool,
    /// Should we print the resource usage.
    #[arg(long, default_value_t = false)]
    print_resource_usage: bool,

    /// Specify the features to activate.
    #[command(flatten)]
    feature: FeaturesSpec,
}

impl TestArgs {
    pub fn run(self, config: &Config) -> anyhow::Result<()> {
        let ws = ops::read_workspace(config.manifest_path(), config).unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(1);
        });

        let resolve = ops::resolve_workspace(&ws)?;
        // TODO: Compute all compilation units and remove duplicates, could be unnecessary in future
        // version of Scarb.

        let features_opts = self.feature.try_into()?;

        let mut compilation_units = ops::generate_compilation_units(&resolve, &features_opts, &ws)?;
        compilation_units.sort_by_key(|unit| unit.main_package_id());
        compilation_units.dedup_by_key(|unit| unit.main_package_id());

        for unit in compilation_units {
            let unit = if let CompilationUnit::Cairo(unit) = unit {
                unit
            } else {
                continue;
            };

            let props: Props = unit.main_component().target_props()?;
            let db = build_root_database(&unit)?;

            if DiagnosticsReporter::stderr().allow_warnings().check(&db) {
                bail!("failed to compile");
            }

            let mut main_crate_ids = collect_main_crate_ids(&unit, &db);
            let test_crate_ids = main_crate_ids.clone();

            if unit.main_package_id.name.to_string() != "dojo" {
                let core_crate_ids = collect_core_crate_ids(&db);
                main_crate_ids.extend(core_crate_ids);
            }

            if let Some(external_contracts) = props.build_external_contracts {
                main_crate_ids.extend(collect_external_crate_ids(&db, external_contracts));
            }

            let config = TestRunConfig {
                filter: self.filter.clone(),
                ignored: self.ignored,
                include_ignored: self.include_ignored,
                run_profiler: self.profiler_mode.clone().into(),
                gas_enabled: self.gas_enabled,
                print_resource_usage: self.print_resource_usage,
            };

            let compiler =
                TestCompiler { db: db.snapshot(), main_crate_ids, test_crate_ids, starknet: true };

            let runner = CompiledTestRunner { compiled: compiler.build()?, config };

            // Database is required here for the profiler to work.
            runner.run(Some(&db))?;

            println!();
        }

        Ok(())
    }
}

pub(crate) fn build_root_database(unit: &CairoCompilationUnit) -> Result<RootDatabase> {
    let mut b = RootDatabase::builder();
    b.with_project_config(build_project_config(unit)?);
    b.with_cfg(CfgSet::from_iter([Cfg::name("test"), Cfg::kv("target", "test")]));

    b.with_plugin_suite(test_plugin_suite());
    b.with_plugin_suite(dojo_plugin_suite());
    b.with_plugin_suite(starknet_plugin_suite());

    b.build()
}

fn build_project_config(unit: &CairoCompilationUnit) -> Result<ProjectConfig> {
    let crate_roots = unit
        .components
        .iter()
        .filter(|model| !model.package.id.is_core())
        // NOTE: We're taking the first target of each compilation unit, which should always be the
        //       main package source root due to the order maintained by scarb.
        .map(|model| (model.cairo_package_name(), model.targets[0].source_root().into()))
        .collect();

    let corelib =
        unit.core_package_component().map(|c| Directory::Real(c.targets[0].source_root().into()));

    let crates_config = crates_config_for_compilation_unit(unit);

    let content = ProjectConfigContent { crate_roots, crates_config };

    let project_config =
        ProjectConfig { base_path: unit.main_component().package.root().into(), corelib, content };

    trace!(target: LOG_TARGET, ?project_config);

    Ok(project_config)
}
