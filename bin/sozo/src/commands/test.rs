//! Compiles and runs tests for a Dojo project using Scarb.
use std::collections::HashSet;
use std::fs;

use anyhow::{Context, Result};
use cairo_lang_sierra::program::VersionedProgram;
use cairo_lang_test_plugin::{TestCompilation, TestCompilationMetadata};
use cairo_lang_test_runner::{CompiledTestRunner, RunProfilerConfig, TestRunConfig};
use camino::Utf8PathBuf;
use clap::Args;
use scarb_interop::Scarb;
use scarb_metadata::Metadata;
use tracing::trace;

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
    pub fn run(self, scarb_metadata: &Metadata) -> anyhow::Result<()> {
        // TODO: For test command, it's merely passing the arguments
        // as the vec[&str], no extra logic.
        // Do we need a profile for test?
        Scarb::test(&scarb_metadata.workspace.manifest_path, vec![])?;

        Ok(())
    }
}
