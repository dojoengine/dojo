use std::env::{self, current_dir};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_project::migration::world::World;
use dojo_project::{EnvironmentConfig, WorldConfig};
use dotenv::dotenv;
use scarb::core::Config;
use scarb::ops;
use scarb::ui::Verbosity;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,

    #[clap(short, long)]
    #[clap(value_name = "DOJO_ENV")]
    #[clap(help = "Specify the environment to perform the migration on.")]
    env: String,

    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
}

#[tokio::main]
pub async fn run(args: MigrateArgs) -> Result<()> {
    dotenv().ok();

    let MigrateArgs { path, env, .. } = args;

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
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .build()
        .unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });

    let world_config = WorldConfig::from_workspace(&ws).unwrap_or_default();
    let env_config = EnvironmentConfig::from_workspace(env, &ws).unwrap_or_default();

    let world = World::from_path(source_dir.clone(), world_config, env_config).await?;
    let mut migration = world.prepare_for_migration(source_dir)?;
    migration.execute().await?;

    Ok(())
}
