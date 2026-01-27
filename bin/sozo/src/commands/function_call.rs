use anyhow::Result;
use clap::Args;
use dojo_world::config::calldata_decoder;
use sozo_ui::SozoUi;
use starknet::core::types::{BlockId, BlockTag, FunctionCall, StarknetError};
use starknet::core::utils as snutils;
use starknet::providers::{Provider, ProviderError};
use tracing::trace;

use super::options::starknet::StarknetOptions;
use crate::utils::CALLDATA_DOC;

#[derive(Debug, Args)]
#[command(about = "Call a contract function without requiring a Dojo project context.")]
pub struct FunctionCallArgs {
    #[arg(help = "The contract address (in hex or decimal format).")]
    pub contract_address: String,

    #[arg(help = "The name of the entrypoint to call.")]
    pub entrypoint: String,

    #[arg(num_args = 0..)]
    #[arg(help = format!("The calldata to be passed to the function.
{CALLDATA_DOC}"))]
    pub calldata: Vec<String>,

    #[arg(short, long)]
    #[arg(help = "The block ID (could be a hash, a number, 'pending' or 'latest')")]
    pub block_id: Option<String>,

    #[command(flatten)]
    pub starknet: StarknetOptions,
}

impl FunctionCallArgs {
    pub async fn run(self, ui: &SozoUi) -> Result<()> {
        trace!(args = ?self);

        let contract_address = parse_contract_address(&self.contract_address)?;

        let calldata = calldata_decoder::decode_calldata(&self.calldata)?;
        dbg!(&calldata);

        let block_id = if let Some(block_id) = self.block_id {
            dojo_utils::parse_block_id(block_id)?
        } else {
            BlockId::Tag(BlockTag::PreConfirmed)
        };

        let (provider, _) = self.starknet.provider(None)?;

        let res = provider
            .call(
                FunctionCall {
                    contract_address,
                    entry_point_selector: snutils::get_selector_from_name(&self.entrypoint)?,
                    calldata,
                },
                block_id,
            )
            .await;

        match res {
            Ok(output) => {
                ui.print(format!(
                    "[ {} ]",
                    output.iter().map(|o| format!("0x{:#066x}", o)).collect::<Vec<_>>().join(" "),
                ));
            }
            Err(e) => {
                anyhow::bail!(format!(
                    "Error calling entrypoint `{}` on address: {:#066x}\n{}",
                    self.entrypoint,
                    contract_address,
                    match &e {
                        ProviderError::StarknetError(StarknetError::ContractError(e)) => {
                            format!("Contract error: {:?}", format_execution_error(&e.revert_error))
                        }
                        _ => e.to_string(),
                    }
                ));
            }
        };

        Ok(())
    }
}

fn parse_contract_address(value: &str) -> Result<starknet::core::types::Felt> {
    use starknet::core::types::Felt;

    if let Ok(felt) = Felt::from_hex(value) {
        return Ok(felt);
    }

    Felt::from_dec_str(value).map_err(|_| {
        anyhow::anyhow!("Invalid contract address `{value}`. Use hex (0x...) or decimal form.")
    })
}

fn format_execution_error(error: &starknet::core::types::ContractExecutionError) -> String {
    match error {
        starknet::core::types::ContractExecutionError::Message(msg) => msg.clone(),
        starknet::core::types::ContractExecutionError::Nested(inner) => {
            let address = format!("{:#066x}", inner.contract_address);
            let selector = format!("0x{:#066x}", inner.selector);
            let inner_error = format_execution_error(&inner.error);
            format!("Error in contract at {address} when calling {selector}:\n  {inner_error}",)
        }
    }
}
