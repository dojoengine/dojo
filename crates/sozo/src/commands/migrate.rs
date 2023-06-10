use std::env::{self, current_dir};

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Args;
use dotenv::dotenv;
use scarb::core::Config;
use scarb::ops;

use super::options::account::AccountOptions;
use super::options::dojo_metadata_from_workspace;
use super::options::starknet::StarknetOptions;
use super::options::world::WorldOptions;
use super::{ui_verbosity_from_flag, ProfileSpec};
use crate::commands::build::{self, BuildArgs};
use crate::ops::migration;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,

    #[clap(short, long)]
    #[clap(help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,

    #[clap(help = "Specify the profile to use.")]
    #[command(flatten)]
    profile_spec: ProfileSpec,

    #[clap(help = "Logging verbosity.")]
    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity,

    #[command(flatten)]
    world: WorldOptions,

    #[command(flatten)]
    starknet: StarknetOptions,

    #[command(flatten)]
    account: AccountOptions,
}

pub fn run(args: MigrateArgs) -> Result<()> {
    dotenv().ok();

    let MigrateArgs { path, profile_spec, verbose, .. } = args;

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
        .ui_verbosity(ui_verbosity_from_flag(verbose.clone()))
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config)?;

    let profile = profile_spec.determine()?;
    let target_dir = source_dir.join(format!("target/{}", profile.as_str()));

    if !target_dir.join("manifest.json").exists() {
        build::run(BuildArgs { path: Some(source_dir), profile_spec, verbose })?;
    }

    let mut env_metadata = dojo_metadata_from_workspace(&ws)
        .and_then(|dojo_metadata| dojo_metadata.get("env").cloned());

    // If there is an environment-specific metadata, use that, otherwise use the
    // workspace's default environment metadata.
    env_metadata = env_metadata
        .as_ref()
        .and_then(|env_metadata| env_metadata.get(ws.config().profile().as_str()).cloned())
        .or(env_metadata);

    ws.config().tokio_handle().block_on(async {
        let world_address = args.world.address(&ws).ok();
        let provider = args.starknet.provider(env_metadata.as_ref())?;

        let account = args
            .account
            .account(provider, env_metadata.as_ref())
            .await
            .with_context(|| "Problem initializing account for migration.")?;

        migration::execute(world_address, account, target_dir, ws.config()).await
    })?;

    Ok(())
}
