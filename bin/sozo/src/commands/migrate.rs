use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::{Args, ValueEnum};
use colored::*;
use dojo_utils::{self, provider as provider_utils, TxnConfig};
use dojo_world::config::migration_config::ManifestAbiFormat;
use dojo_world::contracts::WorldContract;
use dojo_world::services::IpfsService;
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use sozo_ops::migrate::{Migration, MigrationResult};
use sozo_ui::SozoUi;
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::ipfs::IpfsOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
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
    pub ipfs: IpfsOptions,

    /// Select how ABIs are written in the generated manifest.
    #[arg(long, value_enum, value_name = "FORMAT")]
    pub manifest_abi_format: Option<ManifestAbiFormatArg>,
}

impl MigrateArgs {
    /// Runs the migration.
    pub async fn run(self, scarb_metadata: &Metadata, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        scarb_metadata.ensure_profile_artifacts()?;

        let MigrateArgs { world, starknet, account, ipfs, manifest_abi_format, .. } = self;

        print_banner(ui, scarb_metadata, &starknet).await?;

        ui.title("Evaluate project's state");

        let is_guest = world.guest;

        let (world_diff, account, rpc_url) = utils::get_world_diff_and_account(
            account,
            starknet,
            world,
            scarb_metadata,
            &ui.subsection(),
        )
        .await?;

        let world_address = world_diff.world_info.address;
        let profile_config = scarb_metadata.load_dojo_profile_config()?;

        let mut txn_config: TxnConfig = self.transaction.try_into()?;
        txn_config.wait = true;

        let migration = Migration::new(
            world_diff,
            WorldContract::new(world_address, &account),
            txn_config,
            profile_config.clone(),
            rpc_url,
            is_guest,
        );

        let MigrationResult { mut manifest, has_changes } =
            migration.migrate(ui).await.context("Migration failed.")?;

        let config_format =
            profile_config.migration.as_ref().and_then(|m| m.manifest_abi_format.clone());

        let manifest_abi_format =
            manifest_abi_format.map(ManifestAbiFormat::from).or(config_format).unwrap_or_default();

        manifest.apply_abi_format(manifest_abi_format);

        let ipfs_config =
            ipfs.config().or(profile_config.env.map(|env| env.ipfs_config).unwrap_or(None));

        if let Some(config) = ipfs_config {
            let mut metadata_service = IpfsService::new(config)?;

            migration
                .upload_metadata(ui, &mut metadata_service)
                .await
                .context("Metadata upload failed.")?;
        } else {
            ui.warn_block(
                "IPFS credentials not found. Metadata upload skipped. \
                To upload metadata, configure IPFS credentials in your profile config \
                or environment variables: https://book.dojoengine.org/framework/world/metadata.",
            );
        };

        ui.title("Write manifest");
        scarb_metadata
            .write_dojo_manifest_profile(manifest)
            .context("🪦 Failed to write manifest.")?;

        ui.result("Manifest written.");

        let colored_address = format!("{:#066x}", world_address).green();

        let end_text = if has_changes {
            format!("Migration successful with world at address {}", colored_address)
        } else {
            format!("No changes for world at address {:#066x}", world_address)
        };

        ui.new_line();
        ui.block(end_text);
        ui.new_line();

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum ManifestAbiFormatArg {
    AllInOne,
    PerContract,
}

impl From<ManifestAbiFormatArg> for ManifestAbiFormat {
    fn from(value: ManifestAbiFormatArg) -> Self {
        match value {
            ManifestAbiFormatArg::AllInOne => ManifestAbiFormat::AllInOne,
            ManifestAbiFormatArg::PerContract => ManifestAbiFormat::PerContract,
        }
    }
}

#[derive(Debug, Tabled)]
pub struct Banner {
    pub profile: String,
    pub chain_id: String,
    pub rpc_url: String,
}

/// Prints the migration banner.
async fn print_banner(
    ui: &SozoUi,
    scarb_metadata: &Metadata,
    starknet: &StarknetOptions,
) -> Result<()> {
    let profile_config = scarb_metadata.load_dojo_profile_config()?;
    let (provider, rpc_url) = starknet.provider(profile_config.env.as_ref())?;

    let provider = Arc::new(provider);
    if let Err(e) = provider_utils::health_check_provider(provider.clone()).await {
        ui.debug(format!("Provider: {:?}", provider));
        return Err(e);
    }
    let provider = Arc::try_unwrap(provider).map_err(|_| anyhow!("Failed to unwrap Arc"))?;
    let chain_id = provider.chain_id().await?;
    let chain_id =
        parse_cairo_short_string(&chain_id).with_context(|| "Cannot parse chain_id as string")?;

    let banner = Banner { profile: scarb_metadata.current_profile.clone(), chain_id, rpc_url };

    ui.new_line();
    ui.block(format!("{}", Table::new(&[banner]).with(Style::psql())));
    ui.new_line();

    Ok(())
}
