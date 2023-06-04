use std::env::{self, current_dir};

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use clap::Args;
use dotenv::dotenv;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;
use starknet::accounts::{Account, ConnectedAccount};
use starknet::core::types::{BlockId, BlockTag, StarknetError};
use starknet::providers::{Provider, ProviderError};

use super::build::{self, BuildArgs, ProfileSpec};
use crate::ops::migration::config::{EnvironmentConfig, WorldConfig};
use crate::ops::migration::strategy::{execute_migration, prepare_for_migration};
use crate::ops::migration::world::WorldDiff;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,

    #[clap(short, long)]
    #[clap(help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,

    #[command(flatten)]
    profile_spec: ProfileSpec,
}

// TODO: add verbose flag
// TODO: output the migration plan before executing it
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
        .ui_verbosity(Verbosity::Normal)
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

    let migration_output = ws.config().tokio_handle().block_on(async {
        let migrator =
            env_config.migrator().await.with_context(|| "Failed to initialize migrator account")?;

        migrator
            .provider()
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), migrator.address())
            .await
            .map_err(|e| match e {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    anyhow!("Migrator account doesn't exist: {:#x}", migrator.address())
                }
                _ => anyhow!(e),
            })?;

        config.ui().print("🔍 Building world state...");

        let diff = WorldDiff::from_path(target_dir.clone(), &world_config, &env_config).await?;
        let mut migration = prepare_for_migration(target_dir, diff, world_config)?;

        config.ui().print("🌎 Migrating world...");

        execute_migration(&mut migration, migrator)
            .await
            .map_err(|e| anyhow!(e))
            .with_context(|| "Failed to migrate")
    })?;

    config.ui().print(format!(
        "\n✨ Successfully migrated world at address {:#x}",
        migration_output
            .world
            .as_ref()
            .map(|o| o.contract_address)
            .or(world_config.address)
            .expect("world address must exist"),
    ));

    Ok(())
}
