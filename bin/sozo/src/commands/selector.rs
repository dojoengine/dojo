use anyhow::Result;
use clap::{Args, Subcommand};
use dojo_world::contracts::naming::compute_selector_from_tag;
use dojo_world::contracts::WorldContractReader;
use scarb::core::Config;
use sozo_ops::register;
use starknet::accounts::ConnectedAccount;
use starknet::core::types::{BlockId, BlockTag, Felt};
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
pub struct SelectorArgs {
    #[arg(long)]
    pub tag: String,
}

impl SelectorArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        println!("{:#x}", compute_selector_from_tag(&self.tag));
        Ok(())
    }
}
