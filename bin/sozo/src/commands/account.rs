use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};
use scarb::core::Config;
use sozo_ops::account;
use starknet::core::types::Felt;
use starknet::signers::LocalWallet;
use tracing::trace;

use super::options::signer::SignerOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct AccountArgs {
    #[clap(subcommand)]
    command: AccountCommand,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum AccountCommand {
    #[clap(about = "Create a new account configuration without actually deploying.")]
    New {
        #[clap(flatten)]
        signer: SignerOptions,

        #[clap(long, short, help = "Overwrite the account config file if it already exists")]
        force: bool,

        #[clap(help = "Path to save the account config file")]
        file: PathBuf,
    },

    #[clap(about = "Deploy account contract with a DeployAccount transaction.")]
    Deploy {
        #[clap(flatten)]
        starknet: StarknetOptions,

        #[clap(flatten)]
        signer: SignerOptions,

        #[clap(flatten)]
        transaction: TransactionOptions,

        #[clap(long, help = "Provide transaction nonce manually")]
        nonce: Option<Felt>,

        #[clap(
            long,
            env = "STARKNET_POLL_INTERVAL",
            default_value = "1000",
            help = "Transaction result poll interval in milliseconds"
        )]
        poll_interval: u64,

        #[clap(help = "Path to the account config file")]
        file: PathBuf,

        #[clap(long, help = "Don't wait for user confirmation")]
        no_confirmation: bool,
    },

    #[clap(about = "Fetch account config from an already deployed account contract.")]
    Fetch {
        #[clap(flatten)]
        starknet: StarknetOptions,

        #[clap(long, help = "Overwrite the file if it already exists")]
        force: bool,

        #[clap(long, help = "Path to save the account config file")]
        output: PathBuf,

        #[clap(help = "Contract address")]
        address: Felt,
    },
}

impl AccountArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);
        let env_metadata = utils::load_metadata_from_config(config)?;

        config.tokio_handle().block_on(async {
            match self.command {
                AccountCommand::New { signer, force, file } => {
                    let signer: LocalWallet = signer.signer(env_metadata.as_ref(), false)?;
                    trace!(?signer, force, ?file, "Executing New command.");
                    account::new(signer, force, file).await
                }
                AccountCommand::Deploy {
                    starknet,
                    signer,
                    transaction,
                    nonce,
                    poll_interval,
                    file,
                    no_confirmation,
                } => {
                    let provider = starknet.provider(env_metadata.as_ref())?;
                    let signer = signer.signer(env_metadata.as_ref(), false)?;
                    let txn_action = transaction.to_txn_action()?;
                    let txn_config = transaction.into();
                    trace!(
                        ?starknet,
                        ?signer,
                        ?txn_action,
                        ?nonce,
                        poll_interval,
                        ?file,
                        no_confirmation,
                        "Executing Deploy command."
                    );
                    account::deploy(
                        provider,
                        signer,
                        txn_action,
                        txn_config,
                        nonce,
                        poll_interval,
                        file,
                        no_confirmation,
                    )
                    .await
                }
                AccountCommand::Fetch { starknet, force, output, address } => {
                    trace!(?starknet, force, ?output, ?address, "Executing Fetch command.");
                    let provider = starknet.provider(env_metadata.as_ref())?;
                    account::fetch(provider, force, output, address).await
                }
            }
        })
    }
}
