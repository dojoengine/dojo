use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use dojo_utils::{self, TxnConfig};
use dojo_world::contracts::{WorldContract, WorldContractReader};
use scarb::core::{Config, Workspace};
use sozo_ops::migrate::Migration;
use sozo_scarbext::WorkspaceExt;
use spinoff::{spinner, spinners, Spinner};
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[command(flatten)]
    transaction: TransactionOptions,

    #[command(flatten)]
    world: WorldOptions,

    #[command(flatten)]
    starknet: StarknetOptions,

    #[command(flatten)]
    account: AccountOptions,
}

impl MigrateArgs {
    /// Runs the migration.
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        ws.profile_check()?;
        ws.ensure_profile_artifacts()?;

        let MigrateArgs { world, starknet, account, .. } = self;

        let frames = spinner!(["⛩️ ", "🥷 ", "🗡️ "], 500);
        let mut spinner = Spinner::new(frames, "Evaluating world diff...", None);

        config.tokio_handle().block_on(async {
            let mut txn_config: TxnConfig = self.transaction.into();
            txn_config.wait = true;

            let (world_address, world_diff, account) =
                utils::get_world_diff_and_account(account, starknet, world, &ws).await?;

            let migration = Migration::new(
                world_diff,
                WorldContract::new(world_address, account),
                txn_config,
                ws.load_profile_config()?,
            );

            migration.migrate(&mut spinner).await.context("Migration failed.")
        })
    }
}
