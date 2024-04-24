use anyhow::Result;
use bigdecimal::{BigDecimal, Zero};
use clap::Args;
use num_integer::Integer;
use sozo_ops::account::FeeSetting;
use starknet::macros::felt;
use starknet_crypto::FieldElement;
use tracing::trace;

pub(crate) const LOG_TARGET: &str = "sozo::cli::commands::options::fee";

#[derive(Debug, Args, Clone)]
#[command(next_help_heading = "Fee options")]
pub struct FeeOptions {
    #[clap(long, help = "Maximum transaction fee in Ether (18 decimals)")]
    max_fee: Option<BigDecimal>,

    #[clap(long, help = "Maximum transaction fee in Wei")]
    max_fee_raw: Option<FieldElement>,

    #[clap(long, help = "Only estimate transaction fee without sending transaction")]
    estimate_only: bool,
}

impl FeeOptions {
    pub fn into_setting(self) -> Result<FeeSetting> {
        trace!(
            target: LOG_TARGET, 
            max_fee=?self.max_fee,
            max_fee_raw=?self.max_fee_raw,
            estimate_only=self.estimate_only,
            "Converting FeeOptions into FeeSetting."
        );
        match (self.max_fee, self.max_fee_raw, self.estimate_only) {
            (Some(max_fee), None, false) => {
                let max_fee_felt = bigdecimal_to_felt(&max_fee, 18)?;

                // The user is most likely making a mistake for using a max fee higher than 1 ETH
                if max_fee_felt > felt!("1000000000000000000") {
                    trace!(
                        target: LOG_TARGET, 
                        ?max_fee_felt,
                        "Max fee in Ether is higher than 1 ETH."
                    );
                    anyhow::bail!(
                        "the --max-fee value is too large. --max-fee expects a value in Ether (18 \
                         decimals). Use --max-fee-raw instead to use a raw max_fee amount in Wei."
                    )
                }

                Ok(FeeSetting::Manual(max_fee_felt))
            }
            (None, Some(max_fee_raw), false) => {
                trace!(target: LOG_TARGET, ?max_fee_raw, "Using raw max_fee in Wei.");
                Ok(FeeSetting::Manual(max_fee_raw))
            }
            (None, None, true) => {
                trace!(target: LOG_TARGET, "Only estimating the fee.");
                Ok(FeeSetting::EstimateOnly)
            }
            (None, None, false) => {
                trace!(target: LOG_TARGET, "No fee options specified.");
                Ok(FeeSetting::None)
            }
            _ => Err(anyhow::anyhow!(
                "invalid fee option. At most one of --max-fee, --max-fee-raw, and --estimate-only \
                 can be used."
            )),
        }
    }
}

#[allow(clippy::comparison_chain)]
fn bigdecimal_to_felt<D>(dec: &BigDecimal, decimals: D) -> Result<FieldElement>
where
    D: Into<i64>,
{
    let decimals: i64 = decimals.into();

    // Scale the bigint part up or down
    let (bigint, exponent) = dec.as_bigint_and_exponent();

    let mut biguint = match bigint.to_biguint() {
        Some(value) => value,
        None => {
            trace!(target: LOG_TARGET, "Could not convert bigint to biguint, too many decimal places.");
            anyhow::bail!("too many decimal places")
        }
    };

    if exponent < decimals {
        for _ in 0..(decimals - exponent) {
            biguint *= 10u32;
        }
    } else if exponent > decimals {
        for _ in 0..(exponent - decimals) {
            let (quotient, remainder) = biguint.div_rem(&10u32.into());
            if !remainder.is_zero() {
                trace!(target: LOG_TARGET, "Found non-zero remainder during scaling down.");
                anyhow::bail!("too many decimal places")
            }
            biguint = quotient;
        }
    }
    
    Ok(FieldElement::from_byte_slice_be(&biguint.to_bytes_be())?)
}
