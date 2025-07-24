use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use colored::*;
use dojo_utils::{self, provider as provider_utils, TxnConfig};
use dojo_world::contracts::WorldContract;
use dojo_world::services::IpfsService;
use scarb::core::{Config, Workspace};
use sozo_ops::migrate::{Migration, MigrationResult};
use sozo_ops::migration_ui::MigrationUi;
use sozo_scarbext::WorkspaceExt;
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tracing::{error, trace};

use super::options::account::AccountOptions;
use super::options::ipfs::IpfsOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::verify::VerifyOptions;
use super::options::world::WorldOptions;
use crate::commands::LOG_TARGET;
use crate::utils;

#[derive(Debug, Clone, Args)]
pub struct MigrateArgs {
    #[command(flatten)]
    pub transaction: TransactionOptions,

    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,

    #[command(flatten)]
    pub verify: VerifyOptions,

    #[command(flatten)]
    pub ipfs: IpfsOptions,
}

impl MigrateArgs {
    /// Runs the migration.
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;
        ws.profile_check()?;
        ws.ensure_profile_artifacts()?;

        let MigrateArgs { world, starknet, account, verify, ipfs, .. } = self;

        config.tokio_handle().block_on(async {
            print_banner(&ws, &starknet).await?;

            let mut spinner = MigrationUi::new(Some("Evaluating world diff..."));

            let is_guest = world.guest;

            let (world_diff, account, rpc_url) = utils::get_world_diff_and_account(
                account,
                starknet,
                world,
                &ws,
                &mut Some(&mut spinner),
            )
            .await?;

            let world_address = world_diff.world_info.address;
            let profile_config = ws.load_profile_config()?;

            // Create verification configuration if requested
            let mut txn_config: TxnConfig = self.transaction.try_into()?;
            txn_config.wait = true;

            let migration = Migration::new(
                world_diff,
                WorldContract::new(world_address, &account),
                txn_config,
                profile_config.clone(),
                rpc_url,
                is_guest,
            ).with_verification_config(verify.build()?);

            let MigrationResult { manifest, has_changes, verification_results } =
                migration.migrate(&mut spinner).await.context("Migration failed.")?;

            if let Some(results) = verification_results {
                if !results.is_empty() {
                    let mut verification_spinner = MigrationUi::new(Some("Processing verification results..."));
                    verification_spinner.stop_and_persist_boxed("📊 ", "Contract Verification Results:".bright_cyan().to_string());

                    let mut has_failures = false;
                    for result in &results {
                        let message = result.display_message();
                        let mut result_spinner = MigrationUi::new(None);
                        match result {
                            sozo_ops::migrate::VerificationResult::Failed { .. } => {
                                result_spinner.stop_and_persist_boxed("   ❌ ", message.bright_red().to_string());
                                has_failures = true;
                            }
                            sozo_ops::migrate::VerificationResult::Verified { .. }
                            | sozo_ops::migrate::VerificationResult::AlreadyVerified { .. } => {
                                result_spinner.stop_and_persist_boxed("   ✅ ", message.bright_green().to_string());
                            }
                            sozo_ops::migrate::VerificationResult::Submitted { .. }
                            | sozo_ops::migrate::VerificationResult::Timeout { .. } => {
                                result_spinner.stop_and_persist_boxed("   ⚠️ ", message.bright_yellow().to_string());
                            }
                        }
                    }

                    if has_failures {
                        let mut note_spinner = MigrationUi::new(None);
                        note_spinner.stop_and_persist_boxed(
                            "ℹ️  ",
                            "Note: Verification failures do not affect the migration success.".bright_blue().to_string()
                        );
                    }
                }
            }

            let ipfs_config =
                ipfs.config().or(profile_config.env.map(|env| env.ipfs_config).unwrap_or(None));

            if let Some(config) = ipfs_config {
                let mut metadata_service = IpfsService::new(config)?;

                migration
                    .upload_metadata(&mut spinner, &mut metadata_service)
                    .await
                    .context("Metadata upload failed.")?;
            } else {
                let mut ipfs_spinner = MigrationUi::new(None);
                ipfs_spinner.stop_and_persist_boxed(
                    "⚠️ ",
                    "IPFS credentials not found. Metadata upload skipped. To upload metadata, configure IPFS credentials in your profile config or environment variables: https://book.dojoengine.org/framework/world/metadata.".bright_yellow().to_string()
                );
            };

            spinner.update_text("Writing manifest...");
            ws.write_manifest_profile(manifest).context("🪦 Failed to write manifest.")?;

            let colored_address = format!("{:#066x}", world_address).green();

            let (symbol, end_text) = if has_changes {
                (
                    "⛩️ ",
                    format!(
                        "Migration successful with world at address {}",
                        colored_address
                    ),
                )
            } else {
                (
                    "🪨 ",
                    format!(
                        "No changes for world at address {:#066x}",
                        world_address
                    ),
                )
            };

            spinner.stop_and_persist_boxed(symbol, end_text);

            Ok(())
        })
    }
}

#[derive(Debug, Tabled)]
pub struct Banner {
    pub profile: String,
    pub chain_id: String,
    pub rpc_url: String,
}

/// Prints the migration banner.
async fn print_banner(ws: &Workspace<'_>, starknet: &StarknetOptions) -> Result<()> {
    let profile_config = ws.load_profile_config()?;
    let (provider, rpc_url) = starknet.provider(profile_config.env.as_ref())?;

    let provider = Arc::new(provider);
    if let Err(e) = provider_utils::health_check_provider(provider.clone()).await {
        error!(target: LOG_TARGET,"Provider health check failed during sozo migrate.");
        return Err(e);
    }
    let provider = Arc::try_unwrap(provider).map_err(|_| anyhow!("Failed to unwrap Arc"))?;
    let chain_id = provider.chain_id().await?;
    let chain_id =
        parse_cairo_short_string(&chain_id).with_context(|| "Cannot parse chain_id as string")?;

    let banner = Banner {
        profile: ws.current_profile().expect("Scarb profile should be set.").to_string(),
        chain_id,
        rpc_url,
    };

    let mut banner_spinner = MigrationUi::new(None);
    banner_spinner.stop_and_persist_boxed(
        "📋 ",
        format!("\n{}\n", Table::new(&[banner]).with(Style::psql())),
    );

    Ok(())
}
