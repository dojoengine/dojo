use anyhow::{anyhow, Result};
use clap::Args;
use std::str::FromStr;
use dojo_types::naming;
use dojo_utils::{Invoker, TxnConfig};
use dojo_world::config::calldata_decoder;
use scarb::core::Config;
use sozo_ops::resource_descriptor::ResourceDescriptor;
use sozo_scarbext::WorkspaceExt;
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
#[command(about = "Execute a system with the given calldata.")]
pub struct ExecuteArgs {
    #[arg(
        help = "List of calls to execute. Each call should be in format: <CONTRACT_ADDRESS/TAG>,<ENTRYPOINT>,<ARG1>,<ARG2>,... (ex: dojo_examples:actions,execute,1,2)"
    )]
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

#[derive(Debug)]
pub struct CallArgs {
    pub tag_or_address: ResourceDescriptor, // Contract address or tag
    pub entrypoint: String, // Entrypoint to call
    pub calldata: Option<String>, // Calldata to pass to the entrypoint
}


impl std::str::FromStr for CallArgs {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(anyhow!("Empty call string"));
        }

        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() < 2 {
            return Err(anyhow!("Invalid call format. Expected format: <CONTRACT_NAME>,<ENTRYPOINT_NAME>,<ARG1>,<ARG2>,..."));
        }

        let entrypoint = parts[1].trim();
        if entrypoint.is_empty() {
            return Err(anyhow!("Empty entrypoint"));
        }
        if !entrypoint.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(anyhow!("Invalid entrypoint format. Must contain only alphanumeric characters and underscores"));
        }

        Ok(CallArgs {
            tag_or_address: parts[0].parse()?,
            entrypoint: entrypoint.to_string(),
            calldata: if parts.len() > 2 { Some(parts[2..].join(",")) } else { None },
        })
    }
}

fn resolve_contract_address(
    descriptor: &ResourceDescriptor,
    world_diff: &WorldDiff,
) -> Result<Address> {
    match descriptor {
        ResourceDescriptor::Address(address) => Ok(*address),
        ResourceDescriptor::Tag(tag) => {
            let selector = naming::compute_selector_from_tag(tag);
            world_diff
                .get_contract_address(selector)
                .ok_or_else(|| anyhow!("Contract {descriptor} not found in the world diff."))
        }
        ResourceDescriptor::Name(_) => {
            unimplemented!("Expected to be a resolved tag with default namespace.")
        }
    }
}

impl ExecuteArgs {
    pub fn run(self, config: &Config) -> Result<()> {
        trace!(args = ?self);

        let ws = scarb::ops::read_workspace(config.manifest_path(), config)?;

        let profile_config = ws.load_profile_config()?;

        #[cfg(feature = "walnut")]
        let _walnut_debugger = WalnutDebugger::new_from_flag(
            self.transaction.walnut,
            self.starknet.url(profile_config.env.as_ref())?,
        );

        let txn_config: TxnConfig = self.transaction.try_into()?; // Changed from `into()` to `try_into()` for better error handling

        config.tokio_handle().block_on(async {
            // We could save the world diff computation extracting the account directly from the options.
            let (world_diff, account, _) = utils::get_world_diff_and_account(
                self.account,
                self.starknet.clone(),
                self.world,
                &ws,
                &mut None,
            )
            .await?;

            let mut invoker = Invoker::new(&account, txn_config);

            // Parse the Vec<String> into Vec<CallArgs> using FromStr
            let call_args_list: Vec<CallArgs> = self.calls.iter()
                .map(|s| s.parse())
                .collect::<Result<Vec<_>>>()?;

            for call_args in call_args_list {
                let descriptor = call_args.tag_or_address.ensure_namespace(&profile_config.namespace.default);

                // Checking the contract tag in local manifest
                let contract_address = if let Some(local_address) = ws.get_contract_address(&descriptor) {
                    local_address
                } else {
                    resolve_contract_address(&descriptor, &world_diff)?
                };

                trace!(
                    contract=?descriptor,
                    entrypoint=call_args.entrypoint,
                    calldata=?call_args.calldata,
                    "Executing Execute command."
                );

                let calldata = if let Some(cd) = call_args.calldata {
                    calldata_decoder::decode_calldata(&cd)?
                } else {
                    vec![]
                };

                let call = Call {
                    calldata,
                    to: contract_address,
                    selector: snutils::get_selector_from_name(&call_args.entrypoint)?,
                };

                invoker.add_call(call); // Adding each call to the Invoker
            }

            let tx_result = invoker.invoke().await?; // Invoking the multi-call
            println!("{}", tx_result);
            Ok(())
        })
    }
}