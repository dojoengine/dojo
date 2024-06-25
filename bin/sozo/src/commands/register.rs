use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::contracts::WorldContractReader;
use scarb::core::Config;
use sozo_ops::register;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, FieldElement};
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct RegisterArgs {
    #[command(subcommand)]
    pub command: RegisterCommand,
}

#[derive(Debug, Subcommand)]
pub enum RegisterCommand {
    #[command(about = "Register a model to a world.")]
    Model {
        #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "CLASS_HASH")]
        #[arg(help = "The class hash of the models to register.")]
        models: Vec<FieldElement>,

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

impl RegisterArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);
        let env_metadata = utils::load_metadata_from_config(config)?;

        let (starknet, world, account, transaction, models) = match self.command {
            RegisterCommand::Model { starknet, world, account, transaction, models } => {
                trace!(?models, "Registering models.");
                (starknet, world, account, transaction, models)
            }
        };

        let world_address = world.world_address.unwrap_or_default();
        trace!(?world_address, "Using world address.");

        config.tokio_handle().block_on(async {
            let world =
                utils::world_from_env_metadata(world, account, starknet, &env_metadata, config)
                    .await?;
            let provider = world.account.provider();
            let mut world_reader = WorldContractReader::new(world_address, &provider);
            world_reader.set_block(BlockId::Tag(BlockTag::Pending));

            register::model_register(
                models,
                &world,
                transaction.into(),
                world_reader,
                world_address,
                config,
            )
            .await
        })
    }
}
