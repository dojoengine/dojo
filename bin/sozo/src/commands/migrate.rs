use anyhow::Result;
use clap::Args;
use dojo_lang::scarb_internal::compile_workspace;
use scarb::core::{Config, TargetKind};
use scarb::ops::CompileOpts;

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

        let target_dir = ws.target_dir().path_existent().unwrap();
        let target_dir = target_dir.join(ws.config().profile().as_str());

        if !target_dir.join("manifest.json").exists() {
            compile_workspace(
                config,
                CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
            )?;
        }

        ws.config().tokio_handle().block_on(migration::execute(&ws, self, target_dir))?;

        Ok(())
    }
}
