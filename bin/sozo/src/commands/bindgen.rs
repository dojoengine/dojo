use anyhow::Result;
use clap::{Args, ValueEnum};
use dojo_bindgen::{BuiltinPlugins, PluginManager};
use scarb::core::Config;
use scarb_ui::args::{FeaturesSpec, PackagesFilter};
use sozo_scarbext::WorkspaceExt;

use crate::commands::check_package_dojo_version;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BindingType {
    #[value(name = "typescript")]
    Typescript,
    #[value(name = "typescript-v2")]
    TypescriptV2,
    #[value(name = "recs")]
    Recs,
    #[value(name = "unity")]
    Unity,
    #[value(name = "unrealengine")]
    UnrealEngine,
}

#[derive(Debug, Clone, Args)]
pub struct BindgenArgs {
    /// The type of bindings to generate
    #[arg(value_enum)]
    pub binding_type: BindingType,

    #[arg(long)]
    #[arg(help = "Output directory.", default_value = "bindings")]
    pub bindings_output: String,

    /// Specify the features to activate.
    #[command(flatten)]
    pub features: FeaturesSpec,

    /// Specify packages to generate bindings for.
    #[command(flatten)]
    pub packages: Option<PackagesFilter>,
}

impl BindgenArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        ws.profile_check()?;

        let packages = if let Some(filter) = self.packages {
            filter.match_many(&ws)?.into_iter().collect::<Vec<_>>()
        } else {
            ws.members().collect::<Vec<_>>()
        };

        for p in &packages {
            check_package_dojo_version(&ws, p)?;
        }

        // Convert the binding type to BuiltinPlugins
        let builtin_plugins = match self.binding_type {
            BindingType::Typescript => vec![BuiltinPlugins::Typescript],
            BindingType::TypescriptV2 => vec![BuiltinPlugins::TypeScriptV2],
            BindingType::Recs => vec![BuiltinPlugins::Recs],
            BindingType::Unity => vec![BuiltinPlugins::Unity],
            BindingType::UnrealEngine => vec![BuiltinPlugins::UnrealEngine],
        };

        // Create the plugin manager
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

        // Generate bindings
        config.tokio_handle().block_on(bindgen.generate(None)).expect("Error generating bindings");

        Ok(())
    }
}
