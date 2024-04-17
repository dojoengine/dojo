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

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[command(subcommand)]
    pub command: MigrateCommand,

    #[arg(long, global = true)]
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
}

#[derive(Debug, Subcommand)]
pub enum MigrateCommand {
    #[command(about = "Plan the migration and output the manifests.")]
    Plan,
    #[command(about = "Apply the migration on-chain.")]
    Apply {
        #[command(flatten)]
        transaction: TransactionOptions,
    },
}

impl MigrateArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let env_metadata = if config.manifest_path().exists() {
            dojo_metadata_from_workspace(&ws).env().cloned()
        } else {
            None
        };

        let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
        if !manifest_dir.join(MANIFESTS_DIR).exists() {
            return Err(anyhow!("Build project using `sozo build` first"));
        }

        let MigrateArgs { mut name, world, starknet, account, .. } = self;

        if name.is_none() {
            if let Some(root_package) = ws.root_package() {
                name = Some(root_package.id.name.to_string())
            }
        };

        let (world_address, account, chain_id, rpc_url) =
            config.tokio_handle().block_on(async {
                setup_env(&ws, account, starknet, world, name.as_ref(), env_metadata.as_ref()).await
            })?;

        match self.command {
            MigrateCommand::Plan => config.tokio_handle().block_on(async {
                migration::migrate(
                    &ws,
                    world_address,
                    rpc_url,
                    &account,
                    name,
                    true,
                    TxnConfig::default(),
                )
                .await
            }),
            MigrateCommand::Apply { transaction } => config.tokio_handle().block_on(async {
                let txn_config: TxnConfig = transaction.into();

                migration::migrate(&ws, world_address, rpc_url, &account, name, false, txn_config)
                    .await
            }),
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
    let ui = ws.config().ui();

    let world_address = world.address(env).ok();

    let (account, chain_id, rpc_url) = {
        let provider = starknet.provider(env)?;

        let spec_version = provider.spec_version().await?;

        if spec_version != RPC_SPEC_VERSION {
            return Err(anyhow!(
                "Unsupported Starknet RPC version: {}, expected {}.",
                spec_version,
                RPC_SPEC_VERSION
            ));
        }

        let rpc_url = starknet.url(env)?;

        let chain_id = provider.chain_id().await?;
        let chain_id = parse_cairo_short_string(&chain_id)
            .with_context(|| "Cannot parse chain_id as string")?;

        let mut account = account.account(provider, env).await?;
        account.set_block_id(BlockId::Tag(BlockTag::Pending));

        let address = account.address();

        ui.print(format!("\nMigration account: {address:#x}"));

        if let Some(name) = name {
            ui.print(format!("\nWorld name: {name}"));
        }

        ui.print(format!("\nChain ID: {chain_id}\n"));

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
