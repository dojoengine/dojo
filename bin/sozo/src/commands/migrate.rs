use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use dojo_lang::compiler::MANIFESTS_DIR;
use dojo_world::metadata::{dojo_metadata_from_workspace, Environment};
use dojo_world::migration::TxnConfig;
use katana_rpc_api::starknet::RPC_SPEC_VERSION;
use scarb::core::{Config, Workspace};
use sozo_ops::migration;
use starknet::accounts::{Account, ConnectedAccount, SingleOwnerAccount};
use starknet::core::types::{BlockId, BlockTag, FieldElement, StarknetError};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider, ProviderError};
use starknet::signers::LocalWallet;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use tracing::trace;

pub(crate) const LOG_TARGET: &str = "sozo::cli::commands::migrate";

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[command(subcommand)]
    pub command: MigrateCommand,
}

#[derive(Debug, Subcommand)]
pub enum MigrateCommand {
    #[command(about = "Plan the migration and output the manifests.")]
    Plan {
        #[arg(long)]
        #[arg(help = "Name of the World.")]
        #[arg(long_help = "Name of the World. It's hash will be used as a salt when deploying \
                           the contract to avoid address conflicts.")]
        name: Option<String>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        account: AccountOptions,
    },
    #[command(about = "Apply the migration on-chain.")]
    Apply {
        #[arg(long)]
        #[arg(help = "Name of the World.")]
        #[arg(long_help = "Name of the World. It's hash will be used as a salt when deploying \
                           the contract to avoid address conflicts.")]
        name: Option<String>,

        #[command(flatten)]
        world: WorldOptions,

        #[command(flatten)]
        starknet: StarknetOptions,

        #[command(flatten)]
        account: AccountOptions,

        #[command(flatten)]
        transaction: TransactionOptions,
    },
}

impl MigrateArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(target: LOG_TARGET, command=?self.command, "Executing Migrate command");
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let env_metadata = if config.manifest_path().exists() {
            dojo_metadata_from_workspace(&ws).env().cloned()
        } else {
            trace!(target: LOG_TARGET, "Manifest path does not exist.");
            None
        };

        let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
        if !manifest_dir.join(MANIFESTS_DIR).exists() {
            return Err(anyhow!("Build project using `sozo build` first"));
        }

        match self.command {
            MigrateCommand::Plan { mut name, world, starknet, account } => {
                if name.is_none() {
                    if let Some(root_package) = ws.root_package() {
                        name = Some(root_package.id.name.to_string());
                        trace!(target: LOG_TARGET, name, "Setting Root package name.");
                    }
                };

                config.tokio_handle().block_on(async {
                    let (world_address, account, chain_id, rpc_url) = setup_env(
                        &ws,
                        account,
                        starknet,
                        world,
                        name.as_ref(),
                        env_metadata.as_ref(),
                    )
                    .await?;

                    migration::migrate(
                        &ws,
                        world_address,
                        chain_id,
                        rpc_url,
                        &account,
                        name,
                        true,
                        TxnConfig::default(),
                    )
                    .await
                })
            }
            MigrateCommand::Apply { mut name, world, starknet, account, transaction } => {
                trace!(target: LOG_TARGET, name, "Applying migration.");
                let txn_config: TxnConfig = transaction.into();

                if name.is_none() {
                    if let Some(root_package) = ws.root_package() {
                        name = Some(root_package.id.name.to_string());
                        trace!(target: LOG_TARGET, name, "Setting Root package.");
                    }
                };

                config.tokio_handle().block_on(async {
                    let (world_address, account, chain_id, rpc_url) = setup_env(
                        &ws,
                        account,
                        starknet,
                        world,
                        name.as_ref(),
                        env_metadata.as_ref(),
                    )
                    .await?;

                    migration::migrate(
                        &ws,
                        world_address,
                        chain_id,
                        rpc_url,
                        &account,
                        name,
                        false,
                        txn_config,
                    )
                    .await
                })
            }
        }
    }
}

pub async fn setup_env<'a>(
    ws: &'a Workspace<'a>,
    account: AccountOptions,
    starknet: StarknetOptions,
    world: WorldOptions,
    name: Option<&'a String>,
    env: Option<&'a Environment>,
) -> Result<(
    Option<FieldElement>,
    SingleOwnerAccount<JsonRpcClient<HttpTransport>, LocalWallet>,
    String,
    String,
)> {
    trace!(target: LOG_TARGET, "Setting up environment.");
    let ui = ws.config().ui();

    let world_address = world.address(env).ok();
    trace!(target: LOG_TARGET, ?world_address);

    let (account, chain_id, rpc_url) = {
        let provider = starknet.provider(env)?;
        trace!(target: LOG_TARGET, "Provider initialized.");

        let spec_version = provider.spec_version().await?;
        trace!(target: LOG_TARGET, spec_version);

        if spec_version != RPC_SPEC_VERSION {
            return Err(anyhow!(
                "Unsupported Starknet RPC version: {}, expected {}.",
                spec_version,
                RPC_SPEC_VERSION
            ));
        }

        let rpc_url = starknet.url(env)?;
        trace!(target: LOG_TARGET, ?rpc_url);

        let chain_id = provider.chain_id().await?;
        let chain_id = parse_cairo_short_string(&chain_id)
            .with_context(|| "Cannot parse chain_id as string")?;
        trace!(target: LOG_TARGET, chain_id);

        let mut account = account.account(provider, env).await?;
        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        let address = account.address();

        ui.print(format!("\nMigration account: {address:#x}"));
        if let Some(name) = name {
            ui.print(format!("\nWorld name: {name}\n"));
        }

        match account.provider().get_class_hash_at(BlockId::Tag(BlockTag::Pending), address).await {
            Ok(_) => Ok((account, chain_id, rpc_url)),
            Err(ProviderError::StarknetError(StarknetError::ContractNotFound)) => {
                Err(anyhow!("Account with address {:#x} doesn't exist.", account.address()))
            }
            Err(e) => Err(e.into()),
        }
    }
    .with_context(|| "Problem initializing account for migration.")?;

    Ok((world_address, account, chain_id, rpc_url.to_string()))
}
