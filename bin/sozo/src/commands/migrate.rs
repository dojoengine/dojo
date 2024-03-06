use anyhow::{anyhow, Result};
use clap::Args;
use dojo_lang::compiler::MANIFESTS_DIR;
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::Config;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::ops::migration;

#[derive(Args)]
pub struct MigrateArgs {
    #[arg(short, long)]
    #[arg(help = "Perform a dry run and outputs the plan to be executed.")]
    pub dry_run: bool,

    #[arg(long)]
    #[arg(help = "Name of the World.")]
    #[arg(long_help = "Name of the World. It's hash will be used as a salt when deploying the \
                       contract to avoid address conflicts.")]
    pub name: Option<String>,

    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,

    #[command(flatten)]
    pub transaction: TransactionOptions,
}

impl MigrateArgs {
    pub fn run(mut self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        // If `name` was not specified use package name from `Scarb.toml` file if it exists
        if self.name.is_none() {
            if let Some(root_package) = ws.root_package() {
                self.name = Some(root_package.id.name.to_string());
            }
        }

        let env_metadata = if config.manifest_path().exists() {
            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
        if !manifest_dir.join(MANIFESTS_DIR).exists() {
            return Err(anyhow!("Build project using `sozo build` first"));
        }

        ws.config().tokio_handle().block_on(migration::execute(&ws, self, env_metadata))?;

        Ok(())
    }
}
