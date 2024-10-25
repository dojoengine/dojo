use anyhow::{Context, Result};
use clap::{Args, Parser};
use prettytable::format::consts::FORMAT_NO_LINESEP_WITH_TITLE;
use prettytable::{format, Cell, Row, Table};
use scarb::core::{Config, Package, TargetKind};
use scarb::ops::CompileOpts;
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use tracing::trace;

use crate::commands::check_package_dojo_version;

const BYTECODE_SIZE_LABEL: &str = "Bytecode size [in felts]\n(Sierra, Casm)";
const CONTRACT_CLASS_SIZE_LABEL: &str = "Contract Class size [in bytes]\n(Sierra, Casm)";

const CONTRACT_NAME_LABEL: &str = "Contract";

#[derive(Debug, Args)]
pub struct BuildArgs {
    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript: bool,

    #[arg(long)]
    #[arg(help = "Generate Typescript bindings.")]
    pub typescript_v2: bool,

    #[arg(long)]
    #[arg(help = "Generate Unity bindings.")]
    pub unity: bool,

    #[arg(long)]
    #[arg(help = "Output directory.", default_value = "bindings")]
    pub bindings_output: String,

    #[arg(long, help = "Display statistics about the compiled contracts")]
    pub stats: bool,

    /// Specify the features to activate.
    #[command(flatten)]
    pub features: FeaturesSpec,

    /// Specify packages to build.
    #[command(flatten)]
    pub packages: Option<PackagesFilter>,

    #[arg(long)]
    #[arg(help = "Output the Sierra debug information for the compiled contracts.")]
    pub output_debug_info: bool,
}

impl BuildArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let packages: Vec<Package> = if let Some(filter) = self.packages {
            filter.match_many(&ws)?.into_iter().collect()
        } else {
            ws.members().collect()
        };

        for p in &packages {
            check_package_dojo_version(&ws, p)?;
        }

        let profile_name =
            ws.current_profile().expect("Scarb profile is expected at this point.").to_string();

        trace!(?packages);

        scarb::ops::compile(
            packages.iter().map(|p| p.id).collect(),
            CompileOpts {
                include_target_names: vec![],
                include_target_kinds: vec![],
                exclude_target_kinds: vec![TargetKind::TEST],
                features: self.features.try_into()?,
            },
            &ws,
        )?;

        Ok(())
    }
}

impl Default for BuildArgs {
    fn default() -> Self {
        // use the clap defaults
        let features = FeaturesSpec::parse_from([""]);

        Self {
            features,
            typescript: false,
            typescript_v2: false,
            unity: false,
            bindings_output: "bindings".to_string(),
            stats: false,
            packages: None,
            output_debug_info: false,
        }
    }
}
