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
use dojo_lang::scarb_internal::{cfg_set_from_component, crates_config_for_compilation_unit};
use dojo_world::metadata::dojo_metadata_from_package;
use scarb::compiler::helpers::{collect_all_crate_ids, collect_main_crate_ids};
use scarb::compiler::{CairoCompilationUnit, CompilationUnit, CompilationUnitAttributes};
use scarb::core::{Config, Package, TargetKind};
use scarb::ops::{self, CompileOpts};
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use tracing::trace;

use super::check_package_dojo_version;

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
    features: FeaturesSpec,
    /// Specify packages to test.
    #[command(flatten)]
    pub packages: Option<PackagesFilter>,
}

impl TestArgs {
    pub fn run(self, config: &Config) -> anyhow::Result<()> {
        let ws = ops::read_workspace(config.manifest_path(), config).unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(1);
        });

        let packages: Vec<Package> = if let Some(filter) = self.packages {
            filter.match_many(&ws)?.into_iter().collect()
        } else {
            ws.members().collect()
        };

        for p in &packages {
            check_package_dojo_version(&ws, p)?;
        }

        let resolve = ops::resolve_workspace(&ws)?;

        let opts = CompileOpts {
            include_target_kinds: vec![TargetKind::TEST],
            exclude_target_kinds: vec![],
            include_target_names: vec![],
            features: self.features.try_into()?,
        };

        let compilation_units = ops::generate_compilation_units(&resolve, &opts.features, &ws)?
            .into_iter()
            .filter(|cu| {
                let is_excluded =
                    opts.exclude_target_kinds.contains(&cu.main_component().target_kind());
                let is_included = opts.include_target_kinds.is_empty()
                    || opts.include_target_kinds.contains(&cu.main_component().target_kind());
                let is_included = is_included
                    && (opts.include_target_names.is_empty()
                        || cu
                            .main_component()
                            .targets
                            .iter()
                            .any(|t| opts.include_target_names.contains(&t.name)));

                let is_selected = packages.iter().any(|p| p.id == cu.main_package_id());

                let is_cairo_plugin = matches!(cu, CompilationUnit::ProcMacro(_));
                is_cairo_plugin || (is_included && is_selected && !is_excluded)
            })
            .collect::<Vec<_>>();

        for unit in compilation_units {
            let mut unit = if let CompilationUnit::Cairo(unit) = unit {
                unit
            } else {
                continue;
            };

            config.ui().print(format!("testing {}", unit.name()));

            let root_dojo_metadata = dojo_metadata_from_package(&unit.components[0].package, &ws)?;

            // For each component in the compilation unit (namely, the dependencies being
            // compiled) we inject into the `CfgSet` the component name and
            // namespace configuration. Doing this here ensures the parsing of
            // of the manifest is done once at compile time, and not everytime
            // the plugin is called.
            for c in unit.components.iter_mut() {
                c.cfg_set = Some(cfg_set_from_component(
                    c,
                    &root_dojo_metadata.namespace,
                    &config.ui(),
                    &ws,
                )?);

                // As we override all the components CfgSet to ensure the namespace mapping
                // is effective for all of them, we must also insert the "test" and "target"
                // configs here to ensure correct testing configuration.
                if let Some(cfg_set) = c.cfg_set.as_mut() {
                    cfg_set.insert(Cfg::name("test"));
                    cfg_set.insert(Cfg::kv("target", "test"));
                }
            }

            let props: Props = unit.main_component().target_props()?;
            let db = build_root_database(&unit)?;

            if DiagnosticsReporter::stderr().allow_warnings().check(&db) {
                bail!("failed to compile");
            }

            let mut main_crate_ids = collect_all_crate_ids(&unit, &db);
            let test_crate_ids = collect_main_crate_ids(&unit, &db);

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

            let compiled = compiler.build()?;
            let runner = CompiledTestRunner { compiled, config };

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
        .filter(|c| !c.package.id.is_core())
        // NOTE: We're taking the first target of each compilation unit, which should always be the
        //       main package source root due to the order maintained by scarb.
        .map(|c| (c.cairo_package_name(), c.targets[0].source_root().into()))
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

#[cfg(test)]
mod tests {
    use dojo_test_utils::compiler::CompilerTestSetup;
    use scarb::compiler::Profile;

    use super::*;

    // Ignored as scarb takes too much time to compile in debug mode.
    // It's anyway run in the CI in the `test` job.
    #[test]
    #[ignore]
    fn test_spawn_and_move_test() {
        let setup = CompilerTestSetup::from_examples("../../crates/dojo/core", "../../examples/");

        let config = setup.build_test_config("spawn-and-move", Profile::DEV);

        let test_args = TestArgs {
            filter: String::new(),
            include_ignored: false,
            ignored: false,
            profiler_mode: ProfilerMode::None,
            gas_enabled: true,
            print_resource_usage: false,
            features: FeaturesSpec {
                features: vec![],
                all_features: true,
                no_default_features: false,
            },
            packages: None,
        };

        let result = test_args.run(&config);
        assert!(result.is_ok());
    }
}
