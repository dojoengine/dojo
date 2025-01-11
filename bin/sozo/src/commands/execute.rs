use std::str::FromStr;

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
use starknet_crypto::Felt;
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use super::options::world::WorldOptions;
use crate::utils;

#[derive(Debug, Clone)]
pub struct CallArguments {
    pub tag_or_address: ResourceDescriptor,
    pub entrypoint: String,
    pub calldata: Vec<Felt>,
}

impl FromStr for CallArguments {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts = s.splitn(3, ",").collect::<Vec<_>>();

        if parts.len() < 2 {
            return Err(anyhow!(
                "Expected call format: tag_or_address,entrypoint[,calldata1,...,calldataN]"
            ));
        }

        let tag_or_address = ResourceDescriptor::from_string(parts[0])?;
        let entrypoint = parts[1].to_string();
        let calldata =
            if parts.len() > 2 { calldata_decoder::decode_calldata(parts[2])? } else { vec![] };

        Ok(CallArguments { tag_or_address, entrypoint, calldata })
    }
}

#[derive(Debug, Args)]
#[command(about = "Execute one or several systems with the given calldata.")]
pub struct ExecuteArgs {
    #[arg(num_args = 1..)]
    #[arg(help = "A list of calls to execute.\n
A call is made up of 3 values, separated by a comma (<TAG_OR_ADDRESS>,<ENTRYPOINT>[,<CALLDATA>]):

- <TAG_OR_ADDRESS>: the address or the tag (ex: dojo_examples-actions) of the contract to be \
                  called,

- <ENTRYPOINT>: the name of the entry point to be called,

- <CALLDATA>: the calldata to be passed to the system. 
    
    Comma separated values e.g., 0x12345,128,u256:9999999999.
    Sozo supports some prefixes that you can use to automatically parse some types. The supported \
                  prefixes are:
        - u256: A 256-bit unsigned integer.
        - sstr: A cairo short string.
        - str: A cairo string (ByteArray).
        - int: A signed integer.
        - no prefix: A cairo felt or any type that fit into one felt.

EXAMPLE

   sozo execute 0x1234,run ns-Actions,move,1,2

Executes the run function of the contract at the address 0x1234 without calldata,
and the move function of the ns-Actions contract, with the calldata [1,2].")]
    pub calls: Vec<CallArguments>,

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

            for call in self.calls {
                let descriptor =
                    call.tag_or_address.ensure_namespace(&profile_config.namespace.default);

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

                trace!(
                    contract=?descriptor,
                    entrypoint=call.entrypoint,
                    calldata=?call.calldata,
                    "Executing Execute command."
                );

                invoker.add_call(Call {
                    to: contract_address,
                    selector: snutils::get_selector_from_name(&call.entrypoint)?,
                    calldata: call.calldata,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_arguments_from_str() {
        let res = CallArguments::from_str("0x1234,run").unwrap();
        assert!(res.tag_or_address == ResourceDescriptor::from_string("0x1234").unwrap());
        assert!(res.entrypoint == "run");

        let res = CallArguments::from_str("dojo-Player,run").unwrap();
        assert!(res.tag_or_address == ResourceDescriptor::from_string("dojo-Player").unwrap());
        assert!(res.entrypoint == "run");

        let res = CallArguments::from_str("Player,run").unwrap();
        assert!(res.tag_or_address == ResourceDescriptor::from_string("Player").unwrap());
        assert!(res.entrypoint == "run");

        let res = CallArguments::from_str("0x1234,run,1,2,3").unwrap();
        assert!(res.tag_or_address == ResourceDescriptor::from_string("0x1234").unwrap());
        assert!(res.entrypoint == "run");
        assert!(res.calldata == vec![Felt::ONE, Felt::TWO, Felt::THREE]);

        // missing entry point
        let res = CallArguments::from_str("0x1234");
        assert!(res.is_err());

        // bad tag_or_address format
        let res = CallArguments::from_str("0x12X4,run");
        assert!(res.is_err());
    }
}
