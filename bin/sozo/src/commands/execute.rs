use anyhow::{anyhow, Result};
use clap::Args;
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
        help = "The address or the tag (ex: dojo_examples:actions) of the contract to be executed."
    )]
    pub tag_or_address: ResourceDescriptor,

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

        let descriptor = self.tag_or_address.ensure_namespace(&profile_config.namespace.default);

        #[cfg(feature = "walnut")]
        let walnut_debugger = WalnutDebugger::new_from_flag(
            self.transaction.walnut,
            self.starknet.url(profile_config.env.as_ref())?,
        );

        let txn_config: TxnConfig = self.transaction.try_into()?;

        config.tokio_handle().block_on(async {
            let (contract_address, contracts) = match &descriptor {
                ResourceDescriptor::Address(address) => (Some(*address), Default::default()),
                ResourceDescriptor::Tag(tag) => {
                    let contracts = utils::contracts_from_manifest_or_diff(
                        self.account.clone(),
                        self.starknet.clone(),
                        self.world,
                        &ws,
                        self.diff,
                    )
                    .await?;

                    (contracts.get(tag).map(|c| c.address), contracts)
                }
                ResourceDescriptor::Name(_) => {
                    unimplemented!("Expected to be a resolved tag with default namespace.")
                }
            };

            let contract_address = contract_address.ok_or_else(|| {
                let mut message = format!("Contract {descriptor} not found in the manifest.");
                if self.diff {
                    message.push_str(
                        " Run the command again with `--diff` to force the fetch of data from the \
                         chain.",
                    );
                }
                anyhow!(message)
            })?;

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

            let (provider, _) = self.starknet.provider(profile_config.env.as_ref())?;

            let account = self
                .account
                .account(provider, profile_config.env.as_ref(), &self.starknet, &contracts)
                .await?;

            let invoker = Invoker::new(&account, txn_config);
            let tx_result = invoker.invoke(call).await?;

            if let Some(walnut_debugger) = walnut_debugger {
                walnut_debugger.debug_transaction(&config.ui(), &tx_result)?;
            }

            println!("{}", tx_result);
            Ok(())
        })
    }
}
