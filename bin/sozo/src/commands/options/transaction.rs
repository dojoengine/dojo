use clap::Args;
use dojo_world::migration::TxConfig;

#[derive(Debug, Args, Clone)]
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
    #[arg(help = "Wait until the transaction is accepted by the sequencer, returning the receipt.")]
    #[arg(long_help = "Wait until the transaction is accepted by the sequencer, returning the \
                       receipt. This will poll the transaction status until it gets accepted or \
                       rejected by the sequencer.")]
    pub wait: bool,
}

impl From<TransactionOptions> for TxConfig {
    fn from(value: TransactionOptions) -> Self {
        Self { fee_estimate_multiplier: value.fee_estimate_multiplier }
    }
}
