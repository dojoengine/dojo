use anyhow::{bail, Result};
use clap::Args;
use dojo_utils::{FeeConfig, TxnAction, TxnConfig};

#[derive(Debug, Clone, Args, Default)]
#[command(next_help_heading = "Transaction options")]
pub struct TransactionOptions {
    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L1 gas amount.")]
    #[arg(global = true)]
    pub gas: Option<u64>,

    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L1 gas price in STRK.")]
    #[arg(global = true)]
    pub gas_price: Option<u128>,

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

    #[arg(long)]
    #[arg(help = "Display the link to debug the transaction with Walnut.")]
    #[arg(global = true)]
    pub walnut: bool,
}

impl TransactionOptions {
    pub fn init_wait() -> Self {
        TransactionOptions { wait: true, ..Default::default() }
    }

    pub fn to_txn_action(&self, simulate: bool, estimate_only: bool) -> Result<TxnAction> {
        match (estimate_only, simulate) {
            (true, true) => {
                bail!("Both `--estimate-only` and `--simulate` cannot be used at same time.")
            }
            (true, false) => Ok(TxnAction::Estimate),
            (false, true) => Ok(TxnAction::Simulate),
            (false, false) => Ok(TxnAction::Send {
                wait: self.wait || self.walnut,
                receipt: self.receipt,
                fee_config: FeeConfig { gas: self.gas, gas_price: self.gas_price },
                walnut: self.walnut,
            }),
        }
    }
}

impl TryFrom<TransactionOptions> for TxnConfig {
    type Error = anyhow::Error;

    fn try_from(value: TransactionOptions) -> Result<Self> {
        Ok(Self {
            wait: value.wait || value.walnut,
            receipt: value.receipt,
            walnut: value.walnut,
            fee_config: FeeConfig { gas: value.gas, gas_price: value.gas_price },
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    #[test]
    fn test_strk_conversion() -> Result<()> {
        let opts = TransactionOptions {
            wait: true,
            receipt: true,
            gas: Some(1000),
            gas_price: Some(100),
            walnut: false,
        };

        let config: TxnConfig = opts.try_into()?;

        assert!(config.wait);
        assert!(config.receipt);
        assert!(!config.walnut);

        assert_eq!(config.fee_config.gas, Some(1000));
        assert_eq!(config.fee_config.gas_price, Some(100));

        Ok(())
    }
}
