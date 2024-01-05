use anyhow::Error;
use std::str::FromStr;

use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::metadata::dojo_metadata_from_workspace;
use scarb::core::Config;
use starknet::core::types::FieldElement;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::ops::register;

#[derive(Clone, Debug)]
pub struct Model {
    pub name: String,
    pub class_hash: FieldElement,
}

impl FromStr for Model {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 2 {
            return Err(Error::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid input",
            )));
        }
        let name = parts[0].to_string();
        let class_hash = parts[1].parse()?;
        Ok(Model { name, class_hash })
    }
}

#[derive(Debug, Args)]
pub struct RegisterArgs {
    #[command(subcommand)]
    pub command: RegisterCommand,
}

#[derive(Debug, Subcommand)]
pub enum RegisterCommand {
    #[command(about = "Register a model to a world.")]
    Model {
        // #[arg(num_args = 1..)]
        #[arg(required = true)]
        #[arg(value_name = "MODEL")]
        #[arg(help = "A (name, class hash) tuple of the models to register.")]
        models: Vec<Model>,

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
        let env_metadata = if config.manifest_path().exists() {
            let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

            // TODO: Check the updated scarb way to read profile specific values
            dojo_metadata_from_workspace(&ws).and_then(|inner| inner.env().cloned())
        } else {
            None
        };

        config.tokio_handle().block_on(register::execute(self.command, env_metadata))
    }
}
