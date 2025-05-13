//! Compiles and runs tests for a Dojo project using Scarb.
use std::collections::HashSet;
use std::fs;

use anyhow::{Context, Result};
use cairo_lang_sierra::program::VersionedProgram;
use cairo_lang_test_plugin::{TestCompilation, TestCompilationMetadata};
use cairo_lang_test_runner::{CompiledTestRunner, RunProfilerConfig, TestRunConfig};
use camino::Utf8PathBuf;
use clap::Args;
use tracing::trace;

use scarb_interop::{Config, Scarb};

//use super::check_package_dojo_version;

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
    /* TODO RBA
       /// Specify the features to activate.
       #[command(flatten)]
       features: FeaturesSpec,
       /// Specify packages to test.
       #[command(flatten)]
       pub packages: PackagesFilter,
    */
}

impl TestArgs {
    pub fn run(self, config: &Config) -> anyhow::Result<()> {
        /* TODO RBA
               let ws = ops::read_workspace(config.manifest_path(), config).unwrap_or_else(|err| {
                   eprintln!("error: {err}");
                   std::process::exit(1);
               });

               // The scarb path is expected to be set in the env variable $SCARB.
               // However, in some installation, we may not have the correct version, which will
               // ends up in an error like "Scarb metadata not found".
               let scarb_cairo_version = scarb::version::get().cairo.version.to_string();
               let scarb_env_value = std::env::var("SCARB").unwrap_or_default();
               let metadata = MetadataCommand::new()
                   .manifest_path(config.manifest_path())
                   .exec()
                   .with_context(|| {
                       format!(
                           "Failed to get scarb metadata. Ensure `$SCARB` is set to the correct path \
                            with the same version of Cairo ({scarb_cairo_version}). Current value: \
                            {scarb_env_value}."
                       )
                   })?;

               let packages = self.packages.match_many(&ws)?;
               for p in &packages {
                   check_package_dojo_version(&ws, p)?;
               }

               let matched = self.packages.match_many(&metadata)?;

               let target_names = matched
                   .iter()
                   .flat_map(|package| {
                       find_testable_targets(package).iter().map(|t| t.name.clone()).collect::<Vec<_>>()
                   })
                   .collect::<Vec<_>>();

               trace!(?target_names, "Extracting testable targets.");
        */
        Scarb::build(config)?;
        Scarb::test(config)?;

        /* TODO RBA
                scarb::ops::compile(
                    packages.iter().map(|p| p.id).collect(),
                    CompileOpts {
                        include_target_kinds: vec![TargetKind::TEST],
                        exclude_target_kinds: vec![],
                        include_target_names: vec![],
                        features: self.features.try_into()?,
                        ignore_cairo_version: false,
                    },
                    &ws,
                )?;

                let target_dir = Utf8PathBuf::from(ws.target_dir_profile().to_string());

                let mut deduplicator = TargetGroupDeduplicator::default();
                for package in matched {
                    println!("testing {} ...", package.name);
                    for target in find_testable_targets(&package) {
                        if !target_names.contains(&target.name) {
                            continue;
                        }
                        let name = target
                            .params
                            .get("group-id")
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string)
                            .unwrap_or(target.name.clone());
                        let already_seen = deduplicator.visit(package.name.clone(), name.clone());
                        if already_seen {
                            continue;
                        }
                        let test_compilation = deserialize_test_compilation(&target_dir, name.clone())?;
                        let config = TestRunConfig {
                            filter: self.filter.clone(),
                            include_ignored: self.include_ignored,
                            ignored: self.ignored,
                            run_profiler: RunProfilerConfig::None,
                            gas_enabled: is_gas_enabled(&metadata, &package.id, target),
                            print_resource_usage: self.print_resource_usage,
                        };
                        let runner = CompiledTestRunner::new(test_compilation, config);
                        runner.run(None)?;
                        println!();
                    }
                }
        */
        Ok(())
    }
}

/* TODO RBA
fn deserialize_test_compilation(target_dir: &Utf8PathBuf, name: String) -> Result<TestCompilation> {
    let file_path = target_dir.join(format!("{}.test.json", name));
    let test_comp_metadata = serde_json::from_str::<TestCompilationMetadata>(
        &fs::read_to_string(file_path.clone())
            .with_context(|| format!("failed to read file: {file_path}"))?,
    )
    .with_context(|| format!("failed to deserialize compiled tests metadata file: {file_path}"))?;

    let file_path = target_dir.join(format!("{}.test.sierra.json", name));
    let sierra_program = serde_json::from_str::<VersionedProgram>(
        &fs::read_to_string(file_path.clone())
            .with_context(|| format!("failed to read file: {file_path}"))?,
    )
    .with_context(|| format!("failed to deserialize compiled tests sierra file: {file_path}"))?;

    Ok(TestCompilation { sierra_program: sierra_program.into_v1()?, metadata: test_comp_metadata })
}

#[derive(Default)]
struct TargetGroupDeduplicator {
    seen: HashSet<(String, String)>,
}

impl TargetGroupDeduplicator {
    /// Returns true if already visited.
    pub fn visit(&mut self, package_name: String, group_name: String) -> bool {
        !self.seen.insert((package_name, group_name))
    }
}

/// Defines if gas is enabled for a given test target.
fn is_gas_enabled(metadata: &Metadata, package_id: &PackageId, target: &TargetMetadata) -> bool {
    metadata
            .compilation_units
            .iter()
            .find(|cu| {
                cu.package == *package_id && cu.target.kind == "test" && cu.target.name == target.name
            })
            .map(|cu| cu.compiler_config.clone())
            .and_then(|c| {
                c.as_object()
                    .and_then(|c| c.get("enable_gas").and_then(|x| x.as_bool()))
            })
            // Defaults to true, meaning gas enabled - relies on cli config then.
            .unwrap_or(true)
}

/// Finds all testable targets in a package.
fn find_testable_targets(package: &PackageMetadata) -> Vec<&TargetMetadata> {
    package.targets.iter().filter(|target| target.kind == "test").collect()
}
 */
