//! Compiles and runs tests for a Dojo project.
use clap::Args;
use scarb::compiler::ContractSelector;
use scarb::core::{Config, Package};
use scarb::ops;
use scarb::ops::{validate_features, FeaturesOpts};
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use serde::{Deserialize, Serialize};

use super::check_package_dojo_version;

const TEST_COMMAND: &str = "snforge";
const TEST_COMMAND_FIRST_ARG: &str = "test";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Props {
    pub build_external_contracts: Option<Vec<ContractSelector>>,
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
    /// Should we run the profiler.
    #[arg(long, default_value_t = false)]
    profiler: bool,
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
    pub fn run(&self, config: &Config) -> anyhow::Result<()> {
        let ws = ops::read_workspace(config.manifest_path(), config).unwrap_or_else(|err| {
            eprintln!("error: {err}");
            std::process::exit(1);
        });

        let packages: Vec<Package> = if let Some(filter) = &self.packages {
            filter.match_many(&ws)?.into_iter().collect()
        } else {
            ws.members().collect()
        };

        for p in &packages {
            check_package_dojo_version(&ws, p)?;
        }

        let features_opts: FeaturesOpts = self.features.clone().try_into()?;
        validate_features(&packages, &features_opts)?;

        packages.iter().try_for_each(|package| {
            std::process::Command::new(TEST_COMMAND)
                .args(self.build_args())
                .current_dir(package.root())
                .stdout(std::process::Stdio::inherit())
                .output()
                .map_err(|_| {
                    anyhow::anyhow!(
                        "Unable to run `{TEST_COMMAND}` for {}",
                        package.manifest_path()
                    )
                })
                .map(|_| ())
        })
    }

    pub fn build_args(&self) -> Vec<&str> {
        let mut args = vec![TEST_COMMAND_FIRST_ARG];

        if self.include_ignored {
            args.push("--include-ignored");
        }

        if self.ignored {
            args.push("--ignored");
        }

        if self.print_resource_usage {
            args.push("--detailed-resources");
        }

        if self.profiler {
            args.push("--build-profile");
        }

        args
    }
}
