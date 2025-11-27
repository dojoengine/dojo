use anyhow::{Context, Result};
use clap::Args;
use dojo_utils::{Invoker, TxnConfig};
use dojo_world::config::calldata_decoder;
use sozo_ui::SozoUi;
use starknet::core::types::{Call, Felt};
use starknet::core::utils::get_selector_from_name;
use tracing::trace;

use super::options::account::AccountOptions;
use super::options::starknet::StarknetOptions;
use super::options::transaction::TransactionOptions;
use crate::utils::{get_account_from_env, CALLDATA_DOC};

#[derive(Debug, Args)]
#[command(about = "Invoke a contract entrypoint on Starknet. This command does not require the \
                   world context to be loaded. Use the execute command to execute systems in the \
                   world context.")]
pub struct InvokeArgs {
    #[arg(value_name = "CONTRACT_ADDRESS", help = "Target contract address.")]
    pub contract: Felt,

    #[arg(value_name = "ENTRYPOINT", help = "Entrypoint to invoke on the contract.")]
    pub entrypoint: String,

    #[arg(
        value_name = "ARG",
        num_args = 0..,
        help = format!(
            "Calldata elements for the entrypoint (space separated).\n\n{}",
            CALLDATA_DOC
        )
    )]
    pub calldata: Vec<String>,

    #[command(flatten)]
    pub transaction: TransactionOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[command(flatten)]
    pub account: AccountOptions,
}

impl InvokeArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let InvokeArgs { contract, entrypoint, calldata, transaction, starknet, account } = self;

        let account = get_account_from_env(account, &starknet).await?;
        let txn_config: TxnConfig = transaction.try_into()?;

        ui.title(format!("Invoke contract {:#066x}", contract));
        ui.step(format!("Entrypoint: {}", entrypoint));

        let mut decoded = Vec::new();
        for item in calldata {
            let felt_values = calldata_decoder::decode_single_calldata(&item)
                .with_context(|| format!("Failed to parse calldata argument `{item}`"))?;
            decoded.extend(felt_values);
        }

        if decoded.is_empty() {
            ui.verbose("Calldata: <empty>");
        } else {
            ui.verbose(format!("Calldata ({} felt(s))", decoded.len()));
        }

        let selector = get_selector_from_name(&entrypoint)?;
        let call = Call { to: contract, selector, calldata: decoded };

        let invoker = Invoker::new(account, txn_config);
        let result = invoker.invoke(call).await?;

        match result {
            dojo_utils::TransactionResult::Noop => {
                ui.result("Nothing to invoke (noop).");
            }
            dojo_utils::TransactionResult::Hash(hash) => {
                ui.result(format!("Invocation sent.\n  Tx hash   : {hash:#066x}"));
            }
            dojo_utils::TransactionResult::HashReceipt(hash, receipt) => {
                ui.result(format!("Invocation included on-chain.\n  Tx hash   : {hash:#066x}"));
                ui.debug(format!("Receipt: {:?}", receipt));
            }
        }

        Ok(())
    }
}
