use anyhow::Result;
use clap::{Args, Parser};
use dojo_bindgen::{BuiltinPlugins, PluginManager};
use scarb::core::{Config, Package, TargetKind};
use scarb::ops::CompileOpts;
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use sozo_scarbext::WorkspaceExt;
use tracing::debug;

use crate::commands::check_package_dojo_version;

#[derive(Debug, Clone, Args)]
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

        let mut builtin_plugins = vec![];

        if self.typescript {
            builtin_plugins.push(BuiltinPlugins::Typescript);
        }

        if self.typescript_v2 {
            builtin_plugins.push(BuiltinPlugins::TypeScriptV2);
        }

        if self.unity {
            builtin_plugins.push(BuiltinPlugins::Unity);
        }

        // Custom plugins are always empty for now.
        let bindgen = PluginManager {
            profile_name: ws.current_profile().expect("Profile expected").to_string(),
            root_package_name: ws
                .root_package()
                .map(|p| p.id.name.to_string())
                .unwrap_or("NO_ROOT_PACKAGE".to_string()),
            output_path: self.bindings_output.into(),
            manifest_path: config.manifest_path().to_path_buf(),
            plugins: vec![],
            builtin_plugins,
        };

        // TODO: check about the skip migration as now we process the metadata
        // directly during the compilation to get the data we need from it.
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(bindgen.generate(None))
            .expect("Error generating bindings");

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
