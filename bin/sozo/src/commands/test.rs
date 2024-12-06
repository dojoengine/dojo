//! Compiles and runs tests for a Dojo project.
//!
//! We can't use scarb to run tests since our injection will not work.
//! Scarb uses other binaries to run tests. Dojo plugin injection is done in scarb itself.
//! When proc macro will be fully supported, we can switch back to scarb.
use cairo_lang_test_runner::RunProfilerConfig;
use clap::Args;
use scarb::compiler::ContractSelector;
use scarb::core::{Config, Package};
use scarb::ops;
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use serde::{Deserialize, Serialize};

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

        // TODO RBA: build args from TestArgs options
        let args = vec![];

        packages.iter().try_for_each(|package| {
            ops::execute_test_subcommand(package, &args, &ws, self.features.clone()).map(|_| ())
        })
    }
}
