use anyhow::{anyhow, Result};
use clap::Args;
use dojo_utils::{Invoker, TxnConfig};
use dojo_world::config::calldata_decoder;
use scarb::core::Config;
use sozo_ops::resource_descriptor::ResourceDescriptor;
use sozo_scarbext::WorkspaceExt;
#[cfg(feature = "walnut")]
use sozo_walnut::WalnutDebugger;
use starknet::core::types::Call;
use starknet::core::utils as snutils;
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Args)]
#[command(about = "Execute one or several systems with the given calldata.")]
pub struct ExecuteArgs {
    #[arg(num_args = 1..)]
    #[arg(required = true)]
    #[arg(help = "A list of calls to execute, separated by a /.

A call is made up of a <TAG_OR_ADDRESS>, an <ENTRYPOINT> and an optional <CALLDATA>:

- <TAG_OR_ADDRESS>: the address or the tag (ex: dojo_examples-actions) of the contract to be \
                  called,

- <ENTRYPOINT>: the name of the entry point to be called,

- <CALLDATA>: the calldata to be passed to the system. 
    
    Space separated values e.g., 0x12345 128 u256:9999999999.
    Sozo supports some prefixes that you can use to automatically parse some types. The supported \
                  prefixes are:
        - u256: A 256-bit unsigned integer.
        - sstr: A cairo short string.
        - str: A cairo string (ByteArray).
        - int: A signed integer.
        - no prefix: A cairo felt or any type that fit into one felt.

EXAMPLE

   sozo execute 0x1234 run / ns-Actions move 1 2

Executes the run function of the contract at the address 0x1234 without calldata,
and the move function of the ns-Actions contract, with the calldata [1,2].")]
    pub calls: Vec<String>,

    #[arg(long)]
    #[arg(help = "If true, sozo will compute the diff of the world from the chain to translate \
                  tags to addresses.")]
    pub diff: bool,

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

        #[cfg(feature = "walnut")]
        let walnut_debugger = WalnutDebugger::new_from_flag(
            self.transaction.walnut,
            self.starknet.url(profile_config.env.as_ref())?,
        );

        let txn_config: TxnConfig = self.transaction.try_into()?;

        config.tokio_handle().block_on(async {
            let (provider, _) = self.starknet.provider(profile_config.env.as_ref())?;

            let contracts = utils::contracts_from_manifest_or_diff(
                self.account.clone(),
                self.starknet.clone(),
                self.world,
                &ws,
                self.diff,
            )
            .await?;

            let account = self
                .account
                .account(provider, profile_config.env.as_ref(), &self.starknet, &contracts)
                .await?;

            let mut invoker = Invoker::new(&account, txn_config);

            let mut arg_iter = self.calls.into_iter();

            while let Some(arg) = arg_iter.next() {
                let tag_or_address = arg;
                let descriptor = ResourceDescriptor::from_string(&tag_or_address)?
                    .ensure_namespace(&profile_config.namespace.default);

                let contract_address = match &descriptor {
                    ResourceDescriptor::Address(address) => Some(*address),
                    ResourceDescriptor::Tag(tag) => contracts.get(tag).map(|c| c.address),
                    ResourceDescriptor::Name(_) => {
                        unimplemented!("Expected to be a resolved tag with default namespace.")
                    }
                };

                let contract_address = contract_address.ok_or_else(|| {
                    let mut message = format!("Contract {descriptor} not found in the manifest.");
                    if self.diff {
                        message.push_str(
                            " Run the command again with `--diff` to force the fetch of data from \
                             the chain.",
                        );
                    }
                    anyhow!(message)
                })?;

                let entrypoint = arg_iter.next().ok_or_else(|| {
                    anyhow!(
                        "You must specify the entry point of the contract `{tag_or_address}` to \
                         invoke, and optionally the calldata."
                    )
                })?;

                let mut calldata = vec![];
                for arg in &mut arg_iter {
                    let arg = match arg.as_str() {
                        "/" | "-" | "\\" => break,
                        _ => calldata_decoder::decode_single_calldata(&arg)?,
                    };
                    calldata.extend(arg);
                }

                trace!(
                    contract=?descriptor,
                    entrypoint=entrypoint,
                    calldata=?calldata,
                    "Decoded call."
                );

                invoker.add_call(Call {
                    to: contract_address,
                    selector: snutils::get_selector_from_name(&entrypoint)?,
                    calldata,
                });
            }

            let tx_result = invoker.multicall().await?;

            #[cfg(feature = "walnut")]
            if let Some(walnut_debugger) = walnut_debugger {
                walnut_debugger.debug_transaction(&config.ui(), &tx_result)?;
            }

            println!("{}", tx_result);
            Ok(())
        })
    }
}
