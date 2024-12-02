use crate::block::{deserialize_u128, deserialize_u64};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceBounds {
    /// The max amount of the resource that can be used in the tx
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_u64"))]
    pub max_amount: u64,
    /// The max price per unit of this resource for this tx
    #[cfg_attr(feature = "serde", serde(deserialize_with = "deserialize_u128"))]
    pub max_price_per_unit: u128,
}

// Renamed to match the feeder gateway API
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceBoundsMapping {
    #[serde(alias = "L1_GAS")]
    pub l1_gas: ResourceBounds,
    #[serde(alias = "L2_GAS")]
    pub l2_gas: ResourceBounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PriceUnit {
    #[serde(rename = "WEI")]
    Wei,
    #[serde(rename = "FRI")]
    Fri,
}

/// Information regarding the fee and gas usages of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "arbitrary", derive(::arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TxFeeInfo {
    /// The total amount of L1 gas consumed by the transaction.
    pub gas_consumed: u128,
    /// The L1 gas price at the time of the transaction execution.
    pub gas_price: u128,
    /// The fee used by the transaction.
    pub overall_fee: u128,
    /// The type of fee used to pay for the transaction, depending on the transaction type.
    pub unit: PriceUnit,
}
