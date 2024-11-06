use katana_primitives::block::GasPrices;

// TODO: implement a proper gas oracle function - sample the l1 gas and data gas prices
// currently this just return the hardcoded value set from the cli or if not set, the default value.
#[derive(Debug)]
pub struct L1GasOracle {
    gas_prices: GasPrices,
    data_gas_prices: GasPrices,
}

impl L1GasOracle {
    pub fn fixed(gas_prices: GasPrices, data_gas_prices: GasPrices) -> Self {
        Self { gas_prices, data_gas_prices }
    }

    /// Returns the current gas prices.
    pub fn current_gas_prices(&self) -> GasPrices {
        self.gas_prices.clone()
    }

    /// Returns the current data gas prices.
    pub fn current_data_gas_prices(&self) -> GasPrices {
        self.data_gas_prices.clone()
    }
}
