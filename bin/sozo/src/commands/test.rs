//! Compiles and runs tests for a Dojo project using Scarb.
use cairo_lang_runner::profiling::ProfilerConfig;
use clap::Args;
use scarb_interop::Scarb;
use scarb_metadata::Metadata;
use scarb_metadata_ext::{MetadataDojoExt, TestRunner};

use crate::features::FeaturesSpec;

#[derive(Debug, Clone, PartialEq, clap::ValueEnum)]
pub enum ProfilerMode {
    None,
    Cairo,
    Sierra,
}

impl From<ProfilerMode> for ProfilerConfig {
    fn from(mode: ProfilerMode) -> Self {
        match mode {
            ProfilerMode::None | ProfilerMode::Cairo => ProfilerConfig::Cairo,
            ProfilerMode::Sierra => ProfilerConfig::Sierra,
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
    pub features: FeaturesSpec,
    // Specify packages to build.
    /// Packages to run this command on, can be a concrete package name (`foobar`) or
    /// a prefix glob (`foo*`).
    #[arg(short, long, value_delimiter = ',', env = "SCARB_PACKAGES_FILTER")]
    pub packages: Vec<String>,
}

impl TestArgs {
    pub fn run(self, scarb_metadata: &Metadata) -> anyhow::Result<()> {
        let mut extra_args = vec![];

        match scarb_metadata.test_runner()? {
            TestRunner::SnfTestRunner => {
                if self.ignored {
                    extra_args.push("--ignored");
                }
                if self.include_ignored {
                    extra_args.push("--include-ignored");
                }

                if self.print_resource_usage {
                    extra_args.push("--detailed-resources");
                }

                //        profiler_mode: ProfilerMode,
                //        gas_enabled: bool,

                if !self.filter.is_empty() {
                    extra_args.push(&self.filter);
                }
            }
            TestRunner::CairoTestRunner => {
                if self.ignored {
                    extra_args.push("--ignored");
                }
                if self.include_ignored {
                    extra_args.push("--include-ignored");
                }

                if self.print_resource_usage {
                    extra_args.push("--print-resource-usage");
                }

                //        profiler_mode: ProfilerMode,
                //        gas_enabled: bool,

                if !self.filter.is_empty() {
                    extra_args.extend(vec!["-f", &self.filter]);
                }
            }
            TestRunner::NoTestRunner => {
                anyhow::bail!(
                    "No test runner defined for the project. Please add a dev-dependency to a \
                     test runner (cairo_test or snforge_std) to your Scarb.toml"
                );
            }
        }

        // TODO: For test command, it's merely passing the arguments
        // as the vec[&str], no extra logic.
        // Do we need a profile for test?
        Scarb::test(
            &scarb_metadata.workspace.manifest_path,
            &self.packages.join(","),
            self.features.into(),
            extra_args,
        )
    }
}
