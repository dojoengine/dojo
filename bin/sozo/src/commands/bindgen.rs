use anyhow::Result;
use clap::{Args, ValueEnum};
use dojo_bindgen::{BuiltinPlugins, PluginManager};
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BindingTarget {
    #[value(name = "typescript")]
    Typescript,
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
    pub async fn run(self, scarb_metadata: &Metadata) -> Result<()> {
        let manifest_path = scarb_metadata.runtime_manifest.clone();
        let profile_name = scarb_metadata.current_profile.to_string();

        let builtin_plugins = match self.binding_target {
            BindingTarget::Typescript => vec![BuiltinPlugins::Typescript],
            BindingTarget::Recs => vec![BuiltinPlugins::Recs],
            BindingTarget::Unity => vec![BuiltinPlugins::Unity],
            BindingTarget::UnrealEngine => vec![BuiltinPlugins::UnrealEngine],
        };

        let bindgen = PluginManager {
            profile_name,
            root_package_name: scarb_metadata.workspace_package_name()?,
            output_path: self.output_dir.into(),
            manifest_path,
            plugins: vec![],
            builtin_plugins,
        };

        bindgen.generate(None).await?;

        Ok(())
    }
}
