use anyhow::{bail, Result};
use clap::Args;
use dojo_world::migration::{TxnAction, TxnConfig};
use starknet::core::types::FieldElement;
use tracing::trace;

#[derive(Debug, Args)]
#[command(next_help_heading = "Transaction options")]
pub struct TransactionOptions {
    #[arg(long, value_name = "MULTIPLIER")]
    #[arg(help = "The multiplier to use for the fee estimate.")]
    #[arg(long_help = "The multiplier to use for the fee estimate. This value will be used on \
                       the estimated fee which will be used as the max fee for the transaction. \
                       (max_fee = estimated_fee * multiplier)")]
    #[arg(conflicts_with = "max_fee_raw")]
    #[arg(global = true)]
    pub fee_estimate_multiplier: Option<f64>,

    #[arg(long)]
    #[arg(help = "Maximum raw value to be used for fees, in Wei.")]
    #[arg(conflicts_with = "fee_estimate_multiplier")]
    #[arg(global = true)]
    pub max_fee_raw: Option<FieldElement>,

    #[arg(long)]
    #[arg(help = "Wait until the transaction is accepted by the sequencer, returning the status \
                  and hash.")]
    #[arg(long_help = "Wait until the transaction is accepted by the sequencer, returning the \
                       status and the hash. This will poll the transaction status until it gets \
                       accepted or rejected by the sequencer.")]
    #[arg(global = true)]
    pub wait: bool,

    #[arg(long)]
    #[arg(
        help = "If --wait is set, returns the full transaction receipt. Otherwise, it is a no-op."
    )]
    #[arg(global = true)]
    pub receipt: bool,
}

impl TransactionOptions {
    pub fn to_txn_action(&self, simulate: bool, estimate_only: bool) -> Result<TxnAction> {
        match (estimate_only, simulate) {
            (true, true) => {
                bail!("Both `--estimate-only` and `--simulate` cannot be used at same time.")
            }
            (true, false) => Ok(TxnAction::Estimate),
            (false, true) => Ok(TxnAction::Simulate),
            (false, false) => Ok(TxnAction::Send {
                wait: self.wait,
                receipt: self.receipt,
                max_fee_raw: self.max_fee_raw,
                fee_estimate_multiplier: self.fee_estimate_multiplier,
            }),
        }
    }
}

impl From<TransactionOptions> for TxnConfig {
    fn from(value: TransactionOptions) -> Self {
        trace!(
            fee_estimate_multiplier = value.fee_estimate_multiplier,
            wait = value.wait,
            receipt = value.receipt,
            "Converting TransactionOptions to TxnConfig."
        );
        Self {
            fee_estimate_multiplier: value.fee_estimate_multiplier,
            wait: value.wait,
            receipt: value.receipt,
            max_fee_raw: value.max_fee_raw,
        }
    }
}
