//! Compiles and runs tests for a Dojo project.
//!
//! We can't use scarb to run tests since our injection will not work.
//! Scarb uses other binaries to run tests. Dojo plugin injection is done in scarb itself.
//! When proc macro will be fully supported, we can switch back to scarb.
use anyhow::{bail, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_compiler::project::{ProjectConfig, ProjectConfigContent};
use cairo_lang_filesystem::cfg::{Cfg, CfgSet};
use cairo_lang_filesystem::db::{CrateSettings, ExperimentalFeaturesConfig, FilesGroup};
use cairo_lang_filesystem::ids::{CrateId, CrateLongId, Directory};
use cairo_lang_project::AllCratesConfig;
use cairo_lang_starknet::starknet_plugin_suite;
use cairo_lang_test_plugin::{test_plugin_suite, TestsCompilationConfig};
use cairo_lang_test_runner::{CompiledTestRunner, RunProfilerConfig, TestCompiler, TestRunConfig};
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use clap::Args;
use dojo_lang::dojo_plugin_suite;
use itertools::Itertools;
use scarb::compiler::{
    CairoCompilationUnit, CompilationUnit, CompilationUnitAttributes, ContractSelector,
};
use scarb::core::{Config, Package, TargetKind};
use scarb::ops::{self, CompileOpts};
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use tracing::trace;

pub const WORLD_QUALIFIED_PATH: &str = "dojo::world::world_contract::world";

use super::check_package_dojo_version;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Props {
    pub build_external_contracts: Option<Vec<ContractSelector>>,
}

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
    // TODO: move this into the DojoCompiler.
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
            let unit = if let CompilationUnit::Cairo(unit) = unit {
                unit
            } else {
                continue;
            };

            config.ui().print(format!("testing {}", unit.name()));

            let props: Props = unit.main_component().target_props()?;
            let db = build_root_database(&unit)?;

            if DiagnosticsReporter::stderr().allow_warnings().check(&db) {
                bail!("failed to compile");
            }

            let test_crate_ids = collect_main_crate_ids(&unit, &db, false);

            let mut main_crate_ids = collect_all_crate_ids(&unit, &db);

            if let Some(external_contracts) = props.build_external_contracts {
                main_crate_ids.extend(collect_crates_ids_from_selectors(&db, &external_contracts));
            }

            let config = TestRunConfig {
                filter: self.filter.clone(),
                ignored: self.ignored,
                include_ignored: self.include_ignored,
                run_profiler: self.profiler_mode.clone().into(),
                gas_enabled: self.gas_enabled,
                print_resource_usage: self.print_resource_usage,
            };

            let compiler = TestCompiler {
                db: db.snapshot(),
                main_crate_ids,
                test_crate_ids,
                allow_warnings: true,
                config: TestsCompilationConfig {
                    starknet: true,
                    add_statements_functions: false,
                    add_statements_code_locations: false,
                },
            };

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
        .map(|c| {
            (
                c.cairo_package_name(),
                c.first_target().source_root().into(),
            )
        })
        .collect();

    let corelib =
        unit.core_package_component().map(|c| Directory::Real(c.targets[0].source_root().into()));

    let crates_config = crates_config_for_compilation_unit(unit);

    let content = ProjectConfigContent { crate_roots, crates_config };

    let project_config =
        ProjectConfig { base_path: unit.main_component().package.root().into(), corelib, content };

    trace!(?project_config, "Project config built.");

    Ok(project_config)
}

/// Collects the main crate ids for Dojo including the core crates.
pub fn collect_main_crate_ids(
    unit: &CairoCompilationUnit,
    db: &RootDatabase,
    with_dojo_core: bool,
) -> Vec<CrateId> {
    let mut main_crate_ids = scarb::compiler::helpers::collect_main_crate_ids(unit, db);

    if unit.main_package_id.name.to_string() != "dojo" && with_dojo_core {
        let core_crate_ids: Vec<CrateId> = collect_crates_ids_from_selectors(
            db,
            &[ContractSelector(WORLD_QUALIFIED_PATH.to_string())],
        );

        main_crate_ids.extend(core_crate_ids);
    }

    main_crate_ids
}

/// Collects the crate ids containing the given contract selectors.
pub fn collect_crates_ids_from_selectors(
    db: &RootDatabase,
    contract_selectors: &[ContractSelector],
) -> Vec<CrateId> {
    contract_selectors
        .iter()
        .map(|selector| selector.package().into())
        .unique()
        .map(|package_name: SmolStr| db.intern_crate(CrateLongId::Real(package_name)))
        .collect::<Vec<_>>()
}

pub fn collect_all_crate_ids(unit: &CairoCompilationUnit, db: &RootDatabase) -> Vec<CrateId> {
    unit.components
        .iter()
        .map(|component| db.intern_crate(CrateLongId::Real(component.cairo_package_name())))
        .collect()
}

pub fn crates_config_for_compilation_unit(unit: &CairoCompilationUnit) -> AllCratesConfig {
    let crates_config: OrderedHashMap<SmolStr, CrateSettings> = unit
        .components()
        .iter()
        .map(|component| {
            // Ensure experimental features are only enable if required.
            let experimental_features = component.package.manifest.experimental_features.clone();
            let experimental_features = experimental_features.unwrap_or_default();

            (
                component.cairo_package_name(),
                CrateSettings {
                    version: Some(component.package.id.version.clone()),
                    edition: component.package.manifest.edition,
                    experimental_features: ExperimentalFeaturesConfig {
                        negative_impls: experimental_features
                            .contains(&SmolStr::new_inline("negative_impls")),
                        coupons: experimental_features.contains(&SmolStr::new_inline("coupons")),
                    },
                    cfg_set: component.cfg_set.clone(),
                },
            )
        })
        .collect();

    AllCratesConfig { override_map: crates_config, ..Default::default() }
}
