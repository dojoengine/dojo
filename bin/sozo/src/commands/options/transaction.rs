use std::fmt::{Display, Formatter};

use anyhow::{bail, Result};
use clap::builder::PossibleValue;
use clap::{Args, ValueEnum};
use dojo_utils::{EthFeeConfig, FeeConfig, StrkFeeConfig, TxnAction, TxnConfig};
use starknet::core::types::Felt;

#[derive(Debug, Clone, Args, Default)]
#[command(next_help_heading = "Transaction options")]
pub struct TransactionOptions {
    #[arg(long)]
    #[arg(help = "Fee token to use.")]
    #[arg(default_value_t = FeeToken::Strk)]
    #[arg(global = true)]
    pub fee: FeeToken,

    #[arg(help_heading = "Transaction options - ETH")]
    #[arg(long, value_name = "MULTIPLIER")]
    #[arg(help = "The multiplier to use for the fee estimate.")]
    #[arg(long_help = "The multiplier to use for the fee estimate. This value will be used on \
                       the estimated fee which will be used as the max fee for the transaction. \
                       (max_fee = estimated_fee * multiplier)")]
    #[arg(conflicts_with_all = ["max_fee_raw", "gas", "gas_price"])]
    #[arg(global = true)]
    pub fee_estimate_multiplier: Option<f64>,

    #[arg(help_heading = "Transaction options - ETH")]
    #[arg(long)]
    #[arg(help = "Maximum raw value to be used for fees, in Wei.")]
    #[arg(conflicts_with_all = ["fee_estimate_multiplier", "gas", "gas_price"])]
    #[arg(global = true)]
    pub max_fee_raw: Option<Felt>,

    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L1 gas amount.")]
    #[arg(conflicts_with_all = ["max_fee_raw", "fee_estimate_multiplier"])]
    #[arg(global = true)]
    pub gas: Option<u64>,

    #[arg(help_heading = "Transaction options - STRK")]
    #[arg(long, help = "Maximum L1 gas price in STRK.")]
    #[arg(conflicts_with_all = ["max_fee_raw", "fee_estimate_multiplier"])]
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
                fee_config: match self.fee {
                    FeeToken::Strk => {
                        FeeConfig::Strk(StrkFeeConfig { gas: self.gas, gas_price: self.gas_price })
                    }
                    FeeToken::Eth => FeeConfig::Eth(EthFeeConfig {
                        max_fee_raw: self.max_fee_raw,
                        fee_estimate_multiplier: self.fee_estimate_multiplier,
                    }),
                },
                walnut: self.walnut,
            }),
        }
    }
}

impl TryFrom<TransactionOptions> for TxnConfig {
    type Error = anyhow::Error;

    fn try_from(value: TransactionOptions) -> Result<Self> {
        match value.fee {
            FeeToken::Eth => {
                if value.gas.is_some() || value.gas_price.is_some() {
                    bail!(
                        "Gas and gas price are not supported for ETH transactions. Use `--fee \
                         strk` instead."
                    );
                }
            }
            FeeToken::Strk => {
                if value.max_fee_raw.is_some() || value.fee_estimate_multiplier.is_some() {
                    bail!(
                        "Max fee raw and fee estimate multiplier are not supported for STRK \
                         transactions. Use `--fee eth` instead."
                    );
                }
            }
        };

        Ok(Self {
            wait: value.wait || value.walnut,
            receipt: value.receipt,
            fee_config: match value.fee {
                FeeToken::Strk => {
                    FeeConfig::Strk(StrkFeeConfig { gas: value.gas, gas_price: value.gas_price })
                }
                FeeToken::Eth => FeeConfig::Eth(EthFeeConfig {
                    max_fee_raw: value.max_fee_raw,
                    fee_estimate_multiplier: value.fee_estimate_multiplier,
                }),
            },
            walnut: value.walnut,
        })
    }
}

#[derive(Debug, Default, Clone)]
pub enum FeeToken {
    #[default]
    Strk,
    Eth,
}

impl ValueEnum for FeeToken {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Eth, Self::Strk]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::Eth => Some(PossibleValue::new("ETH").alias("eth")),
            Self::Strk => Some(PossibleValue::new("STRK").alias("strk")),
        }
    }
}

impl Display for FeeToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eth => write!(f, "ETH"),
            Self::Strk => write!(f, "STRK"),
        }
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
            fee: FeeToken::Strk,
            gas: Some(1000),
            gas_price: Some(100),
            max_fee_raw: None,
            fee_estimate_multiplier: None,
            walnut: false,
        };

        let config: TxnConfig = opts.try_into()?;

        assert!(config.wait);
        assert!(config.receipt);
        assert!(!config.walnut);

        match config.fee_config {
            FeeConfig::Strk(strk_config) => {
                assert_eq!(strk_config.gas, Some(1000));
                assert_eq!(strk_config.gas_price, Some(100));
            }
            _ => panic!("Expected STRK fee config"),
        }

        Ok(())
    }

    #[test]
    fn test_eth_conversion() -> Result<()> {
        let opts = TransactionOptions {
            wait: false,
            receipt: true,
            fee: FeeToken::Eth,
            gas: None,
            gas_price: None,
            max_fee_raw: Some(Felt::from(1000)),
            fee_estimate_multiplier: Some(1.5),
            walnut: true,
        };

        let config: TxnConfig = opts.try_into()?;

        assert!(config.wait);
        assert!(config.receipt);
        assert!(config.walnut);

        match config.fee_config {
            FeeConfig::Eth(eth_config) => {
                assert_eq!(eth_config.max_fee_raw, Some(Felt::from(1000)));
                assert_eq!(eth_config.fee_estimate_multiplier, Some(1.5));
            }
            _ => panic!("Expected ETH fee config"),
        }

        Ok(())
    }

    #[test]
    fn test_invalid_strk_config() {
        let opts = TransactionOptions {
            fee: FeeToken::Strk,
            max_fee_raw: Some(Felt::from(1000)),
            fee_estimate_multiplier: Some(1.5),
            ..Default::default()
        };

        let result: Result<TxnConfig, _> = opts.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_eth_config() {
        let opts = TransactionOptions {
            fee: FeeToken::Eth,
            gas: Some(1000),
            gas_price: Some(100),
            ..Default::default()
        };
        let result: Result<TxnConfig, _> = opts.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn test_fee_token_display() {
        assert_eq!(FeeToken::Eth.to_string(), "ETH");
        assert_eq!(FeeToken::Strk.to_string(), "STRK");
    }
}
