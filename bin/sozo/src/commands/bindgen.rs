use anyhow::Result;
use clap::{Args, ValueEnum};
use dojo_bindgen::{BuiltinPlugins, PluginManager};
use scarb::core::Config;
use sozo_scarbext::WorkspaceExt;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BindingTarget {
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
    /// The target of bindings to generate.
    #[arg(value_enum)]
    #[arg(long)]
    pub binding_target: BindingTarget,

    /// The output directory for the bindings.
    #[arg(long)]
    #[arg(help = "Output directory.", default_value = "bindings")]
    pub output_dir: String,
}

impl BindgenArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        ws.profile_check()?;

        let builtin_plugins = match self.binding_target {
            BindingTarget::Typescript => vec![BuiltinPlugins::Typescript],
            BindingTarget::TypescriptV2 => vec![BuiltinPlugins::TypeScriptV2],
            BindingTarget::Recs => vec![BuiltinPlugins::Recs],
            BindingTarget::Unity => vec![BuiltinPlugins::Unity],
            BindingTarget::UnrealEngine => vec![BuiltinPlugins::UnrealEngine],
        };

        let bindgen = PluginManager {
            profile_name: ws.current_profile().expect("Profile expected").to_string(),
            root_package_name: ws
                .root_package()
                .map(|p| p.id.name.to_string())
                .unwrap_or("NO_ROOT_PACKAGE".to_string()),
            output_path: self.output_dir.into(),
            manifest_path: config.manifest_path().to_path_buf(),
            plugins: vec![],
            builtin_plugins,
        };

        config.tokio_handle().block_on(bindgen.generate(None))?;

        Ok(())
    }
}
