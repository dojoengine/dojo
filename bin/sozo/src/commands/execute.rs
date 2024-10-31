use std::fmt;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use clap::Args;
use dojo_types::naming;
use dojo_utils::Invoker;
use dojo_world::contracts::naming::ensure_namespace;
use scarb::core::Config;
use sozo_scarbext::WorkspaceExt;
use sozo_walnut::WalnutDebugger;
use starknet::core::types::{Call, Felt};
use starknet::core::utils as snutils;
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::commands::calldata_decoder;
use crate::utils;

#[derive(Debug, Args)]
#[command(about = "Execute a system with the given calldata.")]
pub struct ExecuteArgs {
    #[arg(
        help = "The address or the tag (ex: dojo_examples:actions) of the contract to be executed."
    )]
    pub tag_or_address: String,

    #[arg(help = "The name of the entrypoint to be executed.")]
    pub entrypoint: String,

    #[arg(short, long)]
    #[arg(help = "The calldata to be passed to the system. Comma separated values e.g., \
                  0x12345,128,u256:9999999999. Sozo supports some prefixes that you can use to \
                  automatically parse some types. The supported prefixes are:
                  - u256: A 256-bit unsigned integer.
                  - sstr: A cairo short string.
                  - str: A cairo string (ByteArray).
                  - int: A signed integer.
                  - no prefix: A cairo felt or any type that fit into one felt.")]
    pub calldata: Option<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,

    #[command(flatten)]
    pub world: WorldOptions,

    #[command(flatten)]
    pub transaction: TransactionOptions,
}

impl ExecuteArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let profile_config = ws.load_profile_config()?;

        let descriptor = if utils::is_address(&self.tag_or_address) {
            ContractDescriptor::Address(Felt::from_str(&self.tag_or_address)?)
        } else {
            ContractDescriptor::Tag(ensure_namespace(
                &self.tag_or_address,
                &profile_config.namespace.default,
            ))
        };

        #[cfg(feature = "walnut")]
        let _walnut_debugger = WalnutDebugger::new_from_flag(
            self.transaction.walnut,
            self.starknet.url(profile_config.env.as_ref())?,
        );

        config.tokio_handle().block_on(async {
            let (world_diff, account, _) = utils::get_world_diff_and_account(
                self.account,
                self.starknet.clone(),
                self.world,
                &ws,
            )
            .await?;

            let contract_address = match &descriptor {
                ContractDescriptor::Address(address) => Some(*address),
                ContractDescriptor::Tag(tag) => {
                    let selector = naming::compute_selector_from_tag(tag);
                    world_diff.get_contract_address(selector)
                }
            }
            .ok_or_else(|| anyhow!("Contract {descriptor} not found in the world diff."))?;

            let tx_config = self.transaction.into();

            trace!(
                contract=?descriptor,
                entrypoint=self.entrypoint,
                calldata=?self.calldata,
                "Executing Execute command."
            );

            let calldata = if let Some(cd) = self.calldata {
                calldata_decoder::decode_calldata(&cd)?
            } else {
                vec![]
            };

            let call = Call {
                calldata,
                to: contract_address,
                selector: snutils::get_selector_from_name(&self.entrypoint)?,
            };

            let invoker = Invoker::new(&account, tx_config);
            // TODO: add walnut back, perhaps at the invoker level.
            let tx_result = invoker.invoke(call).await?;

            println!("{}", tx_result);
            Ok(())
        })
    }
}

#[derive(Debug)]
pub enum ContractDescriptor {
    Address(Felt),
    Tag(String),
}

impl fmt::Display for ContractDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractDescriptor::Address(address) => write!(f, "{:#066x}", address),
            ContractDescriptor::Tag(tag) => write!(f, "{}", tag),
        }
    }
}
