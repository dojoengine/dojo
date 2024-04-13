use clap::Args;
use dojo_world::migration::TxConfig;

#[derive(Debug, Args)]
#[command(next_help_heading = "Transaction options")]
pub struct TransactionOptions {
    #[arg(long)]
    #[arg(value_name = "MULTIPLIER")]
    #[arg(help = "The multiplier to use for the fee estimate.")]
    #[arg(long_help = "The multiplier to use for the fee estimate. This value will be used on \
                       the estimated fee which will be used as the max fee for the transaction. \
                       (max_fee = estimated_fee * multiplier)")]
    pub fee_estimate_multiplier: Option<f64>,

    #[arg(short, long)]
    #[arg(help = "Wait until the transaction is accepted by the sequencer, returning the status \
                  and hash.")]
    #[arg(long_help = "Wait until the transaction is accepted by the sequencer, returning the \
                       status and the hash. This will poll the transaction status until it gets \
                       accepted or rejected by the sequencer.")]
    pub wait: bool,

    #[arg(short, long)]
    #[arg(
        help = "If --wait is set, returns the full transaction receipt. Otherwise, it is a no-op."
    )]
    #[arg(long_help = "If --wait is set, returns the full transaction receipt. Otherwise, it is \
                       a no-op.")]
    pub receipt: bool,
}

impl From<TransactionOptions> for TxConfig {
    fn from(value: TransactionOptions) -> Self {
        Self {
            fee_estimate_multiplier: value.fee_estimate_multiplier,
            wait: value.wait,
            receipt: value.receipt,
        }
    }
}
