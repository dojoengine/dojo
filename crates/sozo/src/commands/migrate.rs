use std::env::{self, current_dir};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dotenv::dotenv;
use scarb::core::Config;
use scarb::ops;

use super::ui_verbosity_from_flag;
use crate::commands::build::{self, BuildArgs, ProfileSpec};
use crate::ops::migration;
use crate::ops::migration::config::{EnvironmentConfig, WorldConfig};

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,

    #[clap(short, long)]
    #[clap(help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,

    #[command(flatten)]
    profile_spec: ProfileSpec,

    #[clap(help = "Logging verbosity.")]
    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
}

pub fn run(args: MigrateArgs) -> Result<()> {
    dotenv().ok();

    let MigrateArgs { path, profile_spec, .. } = args;

    let source_dir = match path {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut current_path = current_dir().unwrap();
                current_path.push(path);
                Utf8PathBuf::from_path_buf(current_path).unwrap()
            }
        }
        None => Utf8PathBuf::from_path_buf(current_dir().unwrap()).unwrap(),
    };

    let manifest_path = source_dir.join("Scarb.toml");
    let config = Config::builder(manifest_path)
        .ui_verbosity(ui_verbosity_from_flag(args.verbose))
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config)?;

    let profile = profile_spec.determine()?;
    let target_dir = source_dir.join(format!("target/{}", profile.as_str()));

    if !target_dir.join("manifest.json").exists() {
        build::run(BuildArgs { path: Some(source_dir), profile_spec })?;
    }

    let world_config = WorldConfig::from_workspace(&ws).unwrap_or_default();
    let env_config = EnvironmentConfig::from_workspace(profile.as_str(), &ws)?;

    ws.config().tokio_handle().block_on(migration::execute(
        world_config,
        env_config,
        target_dir,
        &ws.config(),
    ))?;

    Ok(())
}
