use anyhow::{bail, Result};
use clap::Args;
use dojo_utils::{FeeConfig, TxnAction, TxnConfig};
use starknet::core::types::TransactionFinalityStatus;

#[derive(Debug, Clone, Args, Default)]
#[command(next_help_heading = "Transaction options")]
pub struct TransactionOptions {
    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L1 gas amount.")]
    #[arg(global = true)]
    pub l1_gas: Option<u64>,

    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L1 gas price in STRK.")]
    #[arg(global = true)]
    pub l1_gas_price: Option<u128>,

    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L1 Data gas amount.")]
    #[arg(global = true)]
    pub l1_data_gas: Option<u64>,

    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L1 Data gas price in STRK.")]
    #[arg(global = true)]
    pub l1_data_gas_price: Option<u128>,

    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L2 gas amount.")]
    #[arg(global = true)]
    pub l2_gas: Option<u64>,

    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L2 gas price in STRK.")]
    #[arg(global = true)]
    pub l2_gas_price: Option<u128>,

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

    #[arg(long)]
    #[arg(help = "Maximum number of calls to send in a single transaction. By default, Sozo \
                  will limit the number of calls to 10.")]
    #[arg(global = true)]
    #[arg(default_value = "10")]
    pub max_calls: Option<usize>,

    #[arg(long)]
    #[arg(help = "The finality status to wait for. Since 0.14, the nodes syncing is sometime \
                  not fast enough to propagate the transaction to the nodes in the \
                  PRE_CONFIRMED state. The default is ACCEPTED_ON_L2. Available options are: \
                  PRE_CONFIRMED, ACCEPTED_ON_L2, ACCEPTED_ON_L1.")]
    #[arg(global = true)]
    #[arg(default_value = "ACCEPTED_ON_L2")]
    pub finality_status: Option<String>,
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
                fee_config: FeeConfig {
                    l1_gas: self.l1_gas,
                    l1_gas_price: self.l1_gas_price,
                    l1_data_gas: self.l1_data_gas,
                    l1_data_gas_price: self.l1_data_gas_price,
                    l2_gas: self.l2_gas,
                    l2_gas_price: self.l2_gas_price,
                },
                walnut: self.walnut,
                max_calls: self.max_calls,
                finality_status: parse_finality_status(self.finality_status.clone())?,
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
            fee_config: FeeConfig {
                l1_gas: value.l1_gas,
                l1_gas_price: value.l1_gas_price,
                l1_data_gas: value.l1_data_gas,
                l1_data_gas_price: value.l1_data_gas_price,
                l2_gas: value.l2_gas,
                l2_gas_price: value.l2_gas_price,
            },
            max_calls: value.max_calls,
            finality_status: parse_finality_status(value.finality_status.clone())?,
        })
    }
}

/// Parses the finality status from a string.
/// If no status is provided, the default is ACCEPTED_ON_L2.
/// # Arguments
///
/// * `status` - The finality status to parse.
///
/// # Returns
///
/// The parsed finality status.
fn parse_finality_status(status: Option<String>) -> Result<TransactionFinalityStatus> {
    if let Some(status) = status {
        match status.to_uppercase().as_str() {
            "PRE_CONFIRMED" => Ok(TransactionFinalityStatus::PreConfirmed),
            "ACCEPTED_ON_L2" => Ok(TransactionFinalityStatus::AcceptedOnL2),
            "ACCEPTED_ON_L1" => Ok(TransactionFinalityStatus::AcceptedOnL1),
            _ => bail!("Invalid finality status: {}", status),
        }
    } else {
        Ok(TransactionFinalityStatus::AcceptedOnL2)
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
            l1_gas: Some(1_000),
            l1_gas_price: Some(100),
            l1_data_gas: Some(20),
            l1_data_gas_price: Some(200),
            l2_gas: Some(10_000),
            l2_gas_price: Some(1_000),
            walnut: false,
            max_calls: Some(10),
            finality_status: Some("PRE_CONFIRMED".to_string()),
        };

        let config: TxnConfig = opts.try_into()?;

        assert!(config.wait);
        assert!(config.receipt);
        assert!(!config.walnut);
        assert_eq!(config.max_calls, Some(10));

        assert_eq!(config.fee_config.l1_gas, Some(1_000));
        assert_eq!(config.fee_config.l1_gas_price, Some(100));
        assert_eq!(config.fee_config.l1_data_gas, Some(20));
        assert_eq!(config.fee_config.l1_data_gas_price, Some(200));
        assert_eq!(config.fee_config.l2_gas, Some(10_000));
        assert_eq!(config.fee_config.l2_gas_price, Some(1_000));

        assert_eq!(config.finality_status, TransactionFinalityStatus::PreConfirmed);

        Ok(())
    }

    #[test]
    fn test_parse_finality_status() -> Result<()> {
        matches!(
            parse_finality_status(Some("PRE_CONFIRMED".to_string())),
            Ok(TransactionFinalityStatus::PreConfirmed)
        );

        matches!(
            parse_finality_status(Some("ACCEPTED_ON_L2".to_string())),
            Ok(TransactionFinalityStatus::AcceptedOnL2)
        );

        matches!(
            parse_finality_status(Some("ACCEPTED_ON_L1".to_string())),
            Ok(TransactionFinalityStatus::AcceptedOnL1)
        );

        matches!(parse_finality_status(None), Ok(TransactionFinalityStatus::AcceptedOnL2));

        assert!(parse_finality_status(Some("INVALID".to_string())).is_err());

        Ok(())
    }
}
