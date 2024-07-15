use starknet::core::types::PriceUnit;

/// Information regarding the fee and gas usages of a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
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
