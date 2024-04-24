use clap::Args;
use dojo_world::migration::TxnConfig;
use starknet::core::types::FieldElement;

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

    #[arg(short, long)]
    #[arg(help = "Maximum raw value to be used for fees, in Wei.")]
    #[arg(conflicts_with = "fee_estimate_multiplier")]
    #[arg(global = true)]
    pub max_fee_raw: Option<FieldElement>,

    #[arg(short, long)]
    #[arg(help = "Wait until the transaction is accepted by the sequencer, returning the status \
                  and hash.")]
    #[arg(long_help = "Wait until the transaction is accepted by the sequencer, returning the \
                       status and the hash. This will poll the transaction status until it gets \
                       accepted or rejected by the sequencer.")]
    #[arg(global = true)]
    pub wait: bool,

    #[arg(short, long)]
    #[arg(
        help = "If --wait is set, returns the full transaction receipt. Otherwise, it is a no-op."
    )]
    #[arg(global = true)]
    pub receipt: bool,
}

impl From<TransactionOptions> for TxnConfig {
    fn from(value: TransactionOptions) -> Self {
        Self {
            fee_estimate_multiplier: value.fee_estimate_multiplier,
            wait: value.wait,
            receipt: value.receipt,
            max_fee_raw: value.max_fee_raw,
        }
    }
}
