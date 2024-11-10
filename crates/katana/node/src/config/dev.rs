use katana_core::constants::{
    DEFAULT_ETH_L1_DATA_GAS_PRICE, DEFAULT_ETH_L1_GAS_PRICE, DEFAULT_STRK_L1_DATA_GAS_PRICE,
    DEFAULT_STRK_L1_GAS_PRICE,
};
use katana_primitives::block::GasPrices;

/// Development configuration.
#[derive(Debug, Clone)]
pub struct DevConfig {
    /// Whether to enable paying fees for transactions.
    ///
    /// If disabled, the transaction's sender will not be charged for the transaction. Any fee
    /// related checks will be skipped.
    ///
    /// For example, if the transaction's fee resources (ie max fee) is higher than the sender's
    /// balance, the transaction will still be considered valid.
    pub fee: bool,

    /// Whether to enable account validation when sending transaction.
    ///
    /// If disabled, the transaction's sender validation logic will not be executed in any
    /// circumstances. Sending a transaction with invalid signatures, will be considered valid.
    ///
    /// In the case where fee estimation or transaction simulation is done *WITHOUT* the
    /// `SKIP_VALIDATE` flag, if validation is disabled, then it would be as if the
    /// estimation/simulation was sent with `SKIP_VALIDATE`. Using `SKIP_VALIDATE` while
    /// validation is disabled is a no-op.
    pub account_validation: bool,

    /// Fixed L1 gas prices for development.
    ///
    /// These are the prices that will be used for calculating the gas fee for transactions.
    pub fixed_gas_prices: Option<FixedL1GasPriceConfig>,
}

/// Fixed gas prices for development.
#[derive(Debug, Clone)]
pub struct FixedL1GasPriceConfig {
    pub gas_price: GasPrices,
    pub data_gas_price: GasPrices,
}

impl std::default::Default for FixedL1GasPriceConfig {
    fn default() -> Self {
        Self {
            gas_price: GasPrices { eth: DEFAULT_ETH_L1_GAS_PRICE, strk: DEFAULT_STRK_L1_GAS_PRICE },
            data_gas_price: GasPrices {
                eth: DEFAULT_ETH_L1_DATA_GAS_PRICE,
                strk: DEFAULT_STRK_L1_DATA_GAS_PRICE,
            },
        }
    }
}

impl std::default::Default for DevConfig {
    fn default() -> Self {
        Self { fee: true, account_validation: true, fixed_gas_prices: None }
    }
}
