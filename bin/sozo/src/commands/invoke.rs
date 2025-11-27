use anyhow::{anyhow, bail, Context, Result};
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
    #[arg(
        num_args = 1..,
        required = true,
        help = format!(
            "Calls to invoke, separated by '/'. \
            Each call follows the format <CONTRACT> <ENTRYPOINT> [CALLDATA...]\n\n{}",
            CALLDATA_DOC
        )
    )]
    pub calls: Vec<String>,

    #[command(flatten)]
    pub transaction: TransactionOptions,

    #[command(flatten)]
    pub starknet: StarknetOptions,

    #[arg(long, default_value = "0x0", help = "Selector for the entrypoint in felt form.")]
    pub selector: Option<Felt>,

    #[command(flatten)]
    #[command(next_help_heading = "Account options")]
    pub account: AccountOptions,
}

impl InvokeArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let account = get_account_from_env(self.account, &self.starknet).await?;
        let txn_config: TxnConfig = self.transaction.try_into()?;
        let mut invoker = Invoker::new(account, txn_config);

        let mut calls_iter = self.calls.into_iter();
        let mut call_index = 0usize;

        while let Some(target) = calls_iter.next() {
            if matches!(target.as_str(), "/" | "-" | "\\") {
                continue;
            }

            let entrypoint = calls_iter.next().ok_or_else(|| {
                anyhow!(
                    "Missing entrypoint for target `{target}`. Provide calls as `<CONTRACT> \
                     <ENTRYPOINT> [CALLDATA...]`."
                )
            })?;

            let contract_address = parse_contract_address(&target)?;
            let selector = get_selector_from_name(&entrypoint)?;

            let mut calldata = Vec::new();
            for arg in calls_iter.by_ref() {
                match arg.as_str() {
                    "/" | "-" | "\\" => break,
                    _ => {
                        let felts =
                            calldata_decoder::decode_single_calldata(&arg).with_context(|| {
                                format!("Failed to parse calldata argument `{arg}`")
                            })?;
                        calldata.extend(felts);
                    }
                }
            }

            call_index += 1;
            ui.step(format!("Call #{call_index}: {entrypoint} @ {:#066x}", contract_address));
            if calldata.is_empty() {
                ui.verbose("  Calldata: <empty>");
            } else {
                ui.verbose(format!("  Calldata ({} felt(s))", calldata.len()));
            }

            invoker.add_call(Call { to: contract_address, selector, calldata });
        }

        if invoker.calls.is_empty() {
            bail!("No calls provided to invoke.");
        }

        let results = invoker.multicall().await?;

        for (idx, result) in results.iter().enumerate() {
            let display_idx = idx + 1;
            match result {
                dojo_utils::TransactionResult::Noop => {
                    ui.result(format!("Call #{display_idx} noop (no transaction sent)."));
                }
                dojo_utils::TransactionResult::Hash(hash) => {
                    ui.result(format!("Call #{display_idx} sent.\n  Tx hash   : {hash:#066x}"));
                }
                dojo_utils::TransactionResult::HashReceipt(hash, receipt) => {
                    ui.result(format!("Call #{display_idx} included.\n  Tx hash   : {hash:#066x}"));
                    ui.debug(format!("Receipt: {:?}", receipt));
                }
            }
        }

        Ok(())
    }
}

fn parse_contract_address(value: &str) -> Result<Felt> {
    if let Ok(felt) = Felt::from_hex(value) {
        return Ok(felt);
    }

    Felt::from_dec_str(value).map_err(|_| {
        anyhow!("Invalid contract address `{value}`. Use hex (0x...) or decimal form.")
    })
}
