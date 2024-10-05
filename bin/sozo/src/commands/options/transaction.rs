use std::fmt::{Display, Formatter};

use anyhow::{bail, Result};
use bigdecimal::{BigDecimal, Zero};
use clap::builder::PossibleValue;
use clap::{Args, ValueEnum};
use dojo_utils::{
    EthManualFeeSetting, FeeSetting, FeeToken, StrkManualFeeSetting, TokenFeeSetting, TxnAction,
    TxnConfig,
};
use num_integer::Integer;
use num_traits::ToPrimitive;
use starknet::core::types::Felt;
use starknet::macros::felt;
use tracing::trace;

#[derive(Debug, Args, Default)]
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

    #[arg(long, help = "Token to pay transaction fees in. Defaults to ETH")]
    fee_token: Option<FeeToken>,

    #[arg(long, alias = "eth-fee", help = "Shorthand for `--fee-token ETH`")]
    eth: bool,
    #[arg(long, alias = "strk-fee", help = "Shorthand for `--fee-token STRK`")]
    strk: bool,

    #[arg(long, help = "Maximum L1 gas amount (only for STRK fee payment)")]
    gas: Option<Felt>,
    #[arg(long, help = "Maximum L1 gas price in STRK (18 decimals) (only for STRK fee payment)")]
    gas_price: Option<BigDecimal>,
    #[arg(long, help = "Maximum L1 gas price in Fri (only for STRK fee payment)")]
    gas_price_raw: Option<Felt>,

    #[clap(long, help = "Maximum transaction fee in Ether (18 decimals)")]
    max_fee: Option<BigDecimal>,
    #[arg(long)]
    #[arg(help = "Maximum raw value to be used for fees in Wei (only for ETH fee payment)")]
    #[arg(conflicts_with = "fee_estimate_multiplier")]
    #[arg(global = true)]
    pub max_fee_raw: Option<Felt>,

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

    // disabled for now since only simulate was supported only for account deploy
    // reenable when all types of transactions are supported
    // #[arg(long, help = "Simulate the transaction only")]
    // simulate: bool,
    #[arg(long, help = "Only estimate transaction fee without sending transaction")]
    #[arg(global = true)]
    estimate_only: bool,
}

impl TransactionOptions {
    pub fn into_setting(self) -> Result<FeeSetting> {
        let fee_token = match (self.fee_token, self.eth, self.strk) {
            (None, false, false) => FeeToken::Eth,
            (Some(fee_token), false, false) => fee_token,
            (None, true, false) => FeeToken::Eth,
            (None, false, true) => FeeToken::Strk,
            _ => anyhow::bail!("invalid fee token options"),
        };

        match fee_token {
            FeeToken::Eth => {
                if self.gas.is_some() {
                    anyhow::bail!(
                        "the --gas option is not allowed when paying fees in ETH. Use --max-fee \
                         or --max-fee-raw instead for setting fees."
                    );
                }
                if self.gas_price.is_some() {
                    anyhow::bail!(
                        "the --gas-price option is not allowed when paying fees in ETH. Use \
                         --max-fee or --max-fee-raw instead for setting fees."
                    );
                }
                if self.gas_price_raw.is_some() {
                    anyhow::bail!(
                        "the --gas-price-raw option is not allowed when paying fees in ETH. Use \
                         --max-fee or --max-fee-raw instead for setting fees."
                    );
                }

                match (self.max_fee, self.max_fee_raw, self.estimate_only) {
                    (Some(max_fee), None, false) => {
                        let max_fee_felt = bigdecimal_to_felt(&max_fee, 18)?;

                        // The user is most likely making a mistake for using a max fee higher than
                        // 1 ETH use `max_fee_raw` to bypass this safety
                        // check
                        if max_fee_felt > felt!("1000000000000000000") {
                            anyhow::bail!(
                                "the --max-fee value is too large. --max-fee expects a value in \
                                 Ether (18 decimals). Use --max-fee-raw instead to use a raw \
                                 max_fee amount in Wei."
                            )
                        }

                        Ok(FeeSetting::Eth(TokenFeeSetting::Send(EthManualFeeSetting {
                            max_fee: max_fee_felt,
                        })))
                    }
                    (None, Some(max_fee_raw), false) => {
                        Ok(FeeSetting::Eth(TokenFeeSetting::Send(EthManualFeeSetting {
                            max_fee: max_fee_raw,
                        })))
                    }
                    (None, None, true) => Ok(FeeSetting::Eth(TokenFeeSetting::EstimateOnly)),
                    (None, None, false) => Ok(FeeSetting::Eth(TokenFeeSetting::None)),
                    _ => Err(anyhow::anyhow!(
                        "invalid fee option. At most one of --max-fee, --max-fee-raw, and \
                         --estimate-only can be used."
                    )),
                }
            }
            FeeToken::Strk => {
                if self.max_fee.is_some() {
                    anyhow::bail!(
                        "the --max-fee option is not allowed when paying fees in STRK. Use --gas, \
                         --gas-price or --gas-price-raw instead for setting fees."
                    );
                }
                if self.max_fee_raw.is_some() {
                    anyhow::bail!(
                        "the --max-fee-raw option is not allowed when paying fees in STRK. Use \
                         --gas, --gas-price or --gas-price-raw instead for setting fees."
                    );
                }

                if self.estimate_only {
                    if self.gas.is_some()
                        || self.gas_price.is_some()
                        || self.gas_price_raw.is_some()
                    {
                        anyhow::bail!(
                            "invalid fee option. --estimate-only cannot be used with --gas, \
                             --gas-price, or --gas-price-raw."
                        )
                    }

                    Ok(FeeSetting::Strk(TokenFeeSetting::EstimateOnly))
                } else {
                    let gas_override = match self.gas {
                        Some(gas) => Some(
                            gas.to_u64()
                                .ok_or_else(|| anyhow::anyhow!("gas amount out of range"))?,
                        ),
                        None => None,
                    };
                    let gas_price_override = match (self.gas_price, self.gas_price_raw) {
                        (Some(gas_price), None) => {
                            let gas_price = bigdecimal_to_felt(&gas_price, 18)?
                                .to_u128()
                                .ok_or_else(|| anyhow::anyhow!("gas price out of range"))?;

                            // The user is most likely making a mistake for using a gas price higher
                            // than 1 STRK
                            // TODO: allow skipping this safety check
                            if gas_price > 1000000000000000000 {
                                anyhow::bail!(
                                    "the --gas-price value is too large. --gas-price expects a \
                                     value in STRK (18 decimals). Use --gas-price instead to use \
                                     a raw gas_price amount in Fri."
                                )
                            }

                            Some(gas_price)
                        }
                        (None, Some(gas_price_raw)) => {
                            let gas_price = gas_price_raw
                                .to_u128()
                                .ok_or_else(|| anyhow::anyhow!("gas price out of range"))?;

                            Some(gas_price)
                        }
                        (Some(_), Some(_)) => anyhow::bail!(
                            "conflicting fee options: --gas-price and --gas-price-raw"
                        ),
                        (None, None) => None,
                    };

                    match (gas_override, gas_price_override) {
                        (None, None) => Ok(FeeSetting::Strk(TokenFeeSetting::None)),
                        (gas_override, gas_price_override) => {
                            Ok(FeeSetting::Strk(TokenFeeSetting::Send(StrkManualFeeSetting {
                                gas: gas_override,
                                gas_price: gas_price_override,
                            })))
                        }
                    }
                }
            }
        }
    }
}

impl TransactionOptions {
    pub fn init_wait() -> Self {
        TransactionOptions { wait: true, ..Default::default() }
    }
}

impl TryFrom<TransactionOptions> for TxnConfig {
    type Error = anyhow::Error;

    fn try_from(value: TransactionOptions) -> Result<Self> {
        trace!(
            fee_estimate_multiplier = value.fee_estimate_multiplier,
            wait = value.wait,
            receipt = value.receipt,
            "Converting TransactionOptions to TxnConfig."
        );
        Ok(Self {
            fee_estimate_multiplier: value.fee_estimate_multiplier,
            wait: value.wait || value.walnut,
            receipt: value.receipt,
            walnut: value.walnut,
            fee_setting: value.into_setting()?,
        })
    }
}

#[allow(clippy::comparison_chain)]
pub fn bigdecimal_to_felt<D>(dec: &BigDecimal, decimals: D) -> Result<Felt>
where
    D: Into<i64>,
{
    let decimals: i64 = decimals.into();

    // Scale the bigint part up or down
    let (bigint, exponent) = dec.as_bigint_and_exponent();

    let mut biguint = match bigint.to_biguint() {
        Some(value) => value,
        None => anyhow::bail!("too many decimal places"),
    };

    if exponent < decimals {
        for _ in 0..(decimals - exponent) {
            biguint *= 10u32;
        }
    } else if exponent > decimals {
        for _ in 0..(exponent - decimals) {
            let (quotient, remainder) = biguint.div_rem(&10u32.into());
            if !remainder.is_zero() {
                anyhow::bail!("too many decimal places")
            }
            biguint = quotient;
        }
    }

    Ok(Felt::from_bytes_be_slice(&biguint.to_bytes_be()))
}
