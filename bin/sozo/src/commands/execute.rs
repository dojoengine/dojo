use anyhow::{anyhow, bail, Result};
use clap::Args;
use dojo_utils::{Invoker, TxnConfig};
use dojo_world::config::calldata_decoder;
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use sozo_ops::resource_descriptor::ResourceDescriptor;
use sozo_ui::SozoUi;
#[cfg(feature = "walnut")]
use sozo_walnut::WalnutDebugger;
use starknet::core::types::Call;
use starknet::core::utils as snutils;
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils::{self, CALLDATA_DOC};

#[derive(Debug, Args)]
#[command(about = "Execute one or several systems in the world context with the given calldata.")]
pub struct ExecuteArgs {
    #[arg(num_args = 1..)]
    #[arg(required = true)]
    #[arg(help = format!("A list of calls to execute, separated by a /.

A call is made up of a <TAG_OR_ADDRESS>, an <ENTRYPOINT> and an optional <CALLDATA>:

- <TAG_OR_ADDRESS>: 
    * the address or the tag of a Dojo contract (ex: dojo_examples-actions) to be called OR
    * the address or the instance name of a Starknet contract (ex: WoodToken) to be called OR
    * 'world' to call the Dojo world.

- <ENTRYPOINT>: the name of the entry point to be called,

- <CALLDATA>: the calldata to be passed to the system.
{CALLDATA_DOC}

EXAMPLE

   sozo execute 0x1234 run / ns-Actions move 1 2

Executes the run function of the contract at the address 0x1234 without calldata,
and the move function of the ns-Actions contract, with the calldata [1,2]."))]
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
    pub async fn run(self, scarb_metadata: &Metadata, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let profile_config = scarb_metadata.load_dojo_profile_config()?;

        #[cfg(feature = "walnut")]
        let walnut_debugger = WalnutDebugger::new_from_flag(
            self.transaction.walnut,
            self.starknet.url(profile_config.env.as_ref())?,
        );

        let txn_config: TxnConfig = self.transaction.try_into()?;

        let (provider, _) = self.starknet.provider(profile_config.env.as_ref())?;

        let contracts = utils::contracts_from_manifest_or_diff(
            self.account.clone(),
            self.starknet.clone(),
            self.world,
            scarb_metadata,
            self.diff,
            ui,
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

            let contract_address = if tag_or_address == "world" {
                match contracts.get(&tag_or_address) {
                    Some(c) => c.address,
                    None => bail!("Unable to find the world address."),
                }
            } else {
                // first, try to find the contract to call among Dojo contracts
                let descriptor = ResourceDescriptor::from_string(&tag_or_address)?
                    .ensure_namespace(&profile_config.namespace.default);

                let mut contract_address = match &descriptor {
                    ResourceDescriptor::Address(address) => Some(*address),
                    ResourceDescriptor::Tag(tag) => contracts.get(tag).map(|c| c.address),
                    ResourceDescriptor::Name(_) => {
                        unimplemented!("Expected to be a resolved tag with default namespace.")
                    }
                };

                // if not found, try to find a Starknet contract matching with the provided
                // contract name.
                if contract_address.is_none() {
                    contract_address = contracts.get(&tag_or_address).map(|c| c.address);
                }

                contract_address.ok_or_else(|| {
                    let mut message = format!("Contract {descriptor} not found in the manifest.");
                    if self.diff {
                        message.push_str(
                            " Run the command again with `--diff` to force the fetch of data from \
                             the chain.",
                        );
                    }
                    anyhow!(message)
                })?
            };

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
                contract=?contract_address,
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

        let txs_results = invoker.multicall().await?;
        for r in &txs_results {
            println!("{}", r);
        }

        #[cfg(feature = "walnut")]
        if let Some(walnut_debugger) = walnut_debugger {
            for tx_result in &txs_results {
                walnut_debugger.debug_transaction(ui, tx_result)?;
            }
        }

        Ok(())
    }
}
