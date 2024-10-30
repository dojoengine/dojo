use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use dojo_utils::{self, TxnConfig};
use dojo_world::contracts::WorldContract;
use scarb::core::Config;
use sozo_ops::migrate::{Migration, MigrationUi};
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
        let mut spinner =
            MigrationUi::Spinner(Spinner::new(frames, "Evaluating world diff...", None));

        config.tokio_handle().block_on(async {
            let mut txn_config: TxnConfig = self.transaction.into();
            txn_config.wait = true;

            let (world_diff, account, rpc_url) =
                utils::get_world_diff_and_account(account, starknet, world, &ws).await?;

            let world_address = world_diff.world_info.address;

            let migration = Migration::new(
                world_diff,
                WorldContract::new(world_address, &account),
                txn_config,
                ws.load_profile_config()?,
                rpc_url,
            );

            let manifest = migration.migrate(&mut spinner).await.context("Migration failed.")?;

            spinner.update_text("Writing manifest...");
            ws.write_manifest_profile(manifest).context("Failed to write manifest.")?;

            let colored_address = format!("{:#066x}", world_address).green();
            let end_text =
                format!("Migration successful with world at address {}", colored_address);

            spinner.stop_and_persist("⛩️ ", Box::leak(end_text.into_boxed_str()));

            Ok(())
        })
    }
}
