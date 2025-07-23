use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use colored::*;
use dojo_utils::{self, provider as provider_utils, TxnConfig};
use dojo_world::contracts::WorldContract;
use dojo_world::services::IpfsService;
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use sozo_ops::migrate::{Migration, MigrationResult};
use sozo_ops::migration_ui::MigrationUi;
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
    pub async fn run(self, scarb_metadata: &Metadata) -> Result<()> {
        trace!(args = ?self);

        scarb_metadata.ensure_profile_artifacts()?;

        let MigrateArgs { world, starknet, account, ipfs, verify, transaction } = self;

        print_banner(scarb_metadata, &starknet).await?;

        let mut spinner = MigrationUi::new(Some("Evaluating world diff..."));

        let is_guest = world.guest;

        let (world_diff, account, rpc_url) = utils::get_world_diff_and_account(
            account,
            starknet,
            world,
            scarb_metadata,
            &mut Some(&mut spinner),
        )
        .await?;

        let world_address = world_diff.world_info.address;
        let profile_config = scarb_metadata.load_dojo_profile_config()?;

        // Create verification configuration if requested
        let verification_config = verify.create_verification_config()?;

        let mut txn_config: TxnConfig = transaction.try_into()?;
        txn_config.wait = true;

        let migration = if let Some(verification_config) = verification_config {
            Migration::with_verification(
                world_diff,
                WorldContract::new(world_address, &account),
                txn_config,
                profile_config.clone(),
                rpc_url,
                is_guest,
                verification_config,
            )
        } else {
            Migration::new(
                world_diff,
                WorldContract::new(world_address, &account),
                txn_config,
                profile_config.clone(),
                rpc_url,
                is_guest,
            )
        };

        let MigrationResult { manifest, has_changes, verification_results } =
            migration.migrate(&mut spinner).await.context("Migration failed.")?;

        // Display verification results if any
        if let Some(results) = verification_results {
            if !results.is_empty() {
                println!();
                println!("{}", "📊 Contract Verification Results:".bright_cyan());

                let mut has_failures = false;
                for result in &results {
                    let message = result.display_message();
                    if result.display_message().contains("❌") {
                        println!("   {}", message.bright_red());
                        has_failures = true;
                    } else if result.display_message().contains("✅") {
                        println!("   {}", message.bright_green());
                    } else {
                        println!("   {}", message.bright_yellow());
                    }
                }

                if has_failures {
                    println!();
                    println!(
                        "{}",
                        "ℹ️  Note: Verification failures do not affect the migration success."
                            .bright_blue()
                    );
                }
                println!();
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
            println!();
            println!(
                    "{}",
                    "IPFS credentials not found. Metadata upload skipped. To upload metadata, configure IPFS credentials in your profile config or environment variables: https://book.dojoengine.org/framework/world/metadata.".bright_yellow()
                );
        };

        spinner.update_text("Writing manifest...");
        scarb_metadata
            .write_dojo_manifest_profile(manifest)
            .context("🪦 Failed to write manifest.")?;

        let colored_address = format!("{:#066x}", world_address).green();

        let (symbol, end_text) = if has_changes {
            ("⛩️ ", format!("Migration successful with world at address {}", colored_address))
        } else {
            ("🪨 ", format!("No changes for world at address {:#066x}", world_address))
        };

        spinner.stop_and_persist_boxed(symbol, end_text);

        Ok(())
    }
}

#[derive(Debug, Tabled)]
pub struct Banner {
    pub profile: String,
    pub chain_id: String,
    pub rpc_url: String,
}

/// Prints the migration banner.
async fn print_banner(scarb_metadata: &Metadata, starknet: &StarknetOptions) -> Result<()> {
    let profile_config = scarb_metadata.load_dojo_profile_config()?;
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

    let banner = Banner { profile: scarb_metadata.current_profile.clone(), chain_id, rpc_url };

    println!();
    println!("{}", Table::new(&[banner]).with(Style::psql()));
    println!();

    Ok(())
}
