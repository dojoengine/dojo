use anyhow::Result;
use clap::{Args, Parser};
use scarb::core::{Config, Package, TargetKind};
use scarb::ops::CompileOpts;
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use sozo_scarbext::WorkspaceExt;
use tracing::debug;

use crate::commands::check_package_dojo_version;

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
        ws.profile_check()?;

        // Ensure we don't have old contracts in the build dir, since the local artifacts
        // guides the migration.
        ws.clean_dir_profile();

        let packages: Vec<Package> = if let Some(filter) = self.packages {
            filter.match_many(&ws)?.into_iter().collect()
        } else {
            ws.members().collect()
        };

        for p in &packages {
            check_package_dojo_version(&ws, p)?;
        }

        debug!(?packages);

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
