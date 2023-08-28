use cubit::f128::math::core::{ln, abs, exp};
use cubit::f128::types::fixed::{Fixed, FixedTrait};

/// A Linear Variable Rate Gradual Dutch Auction (VRGDA) struct.
/// Represents an auction where the price decays linearly based on the target price,
/// decay constant, and per-time-unit rate.
#[derive(Copy, Drop, Serde, starknet::Storage)]
struct LinearVRGDA {
    target_price: Fixed,
    decay_constant: Fixed,
    per_time_unit: Fixed,
}

#[generate_trait]
impl LinearVRGDAImpl of LinearVRGDATrait {
    /// Calculates the target sale time based on the quantity sold.
    ///
    /// # Arguments
    ///
    /// * `sold`: Quantity sold.
    ///
    /// # Returns
    ///
    /// * A `Fixed` representing the target sale time.
    fn get_target_sale_time(self: @LinearVRGDA, sold: Fixed) -> Fixed {
        sold / *self.per_time_unit
    }

    /// Calculates the VRGDA price at a specific time since the auction started.
    ///
    /// # Arguments
    ///
    /// * `time_since_start`: Time since the auction started.
    /// * `sold`: Quantity sold.
    ///
    /// # Returns
    ///
    /// * A `Fixed` representing the price.
    fn get_vrgda_price(self: @LinearVRGDA, time_since_start: Fixed, sold: Fixed) -> Fixed {
        *self.target_price
            * exp(
                *self.decay_constant
                    * (time_since_start
                        - self.get_target_sale_time(sold + FixedTrait::new(1, false)))
            )
    }
}

#[derive(Copy, Drop, Serde, starknet::Storage)]
struct LogisticVRGDA {
    target_price: Fixed,
    decay_constant: Fixed,
    max_sellable: Fixed,
    time_scale: Fixed,
}

// A Logistic Variable Rate Gradual Dutch Auction (VRGDA) struct.
/// Represents an auction where the price decays according to a logistic function,
/// based on the target price, decay constant, max sellable quantity, and time scale.
#[generate_trait]
impl LogisticVRGDAImpl of LogisticVRGDATrait {
    /// Calculates the target sale time using a logistic function based on the quantity sold.
    ///
    /// # Arguments
    ///
    /// * `sold`: Quantity sold.
    ///
    /// # Returns
    ///
    /// * A `Fixed` representing the target sale time.
    fn get_target_sale_time(self: @LogisticVRGDA, sold: Fixed) -> Fixed {
        let logistic_limit = *self.max_sellable + FixedTrait::ONE();
        let logistic_limit_double = logistic_limit * FixedTrait::new_unscaled(2, false);
        abs(
            ln(logistic_limit_double / (sold + logistic_limit) - FixedTrait::ONE())
                / *self.time_scale
        )
    }

    /// Calculates the VRGDA price at a specific time since the auction started,
    /// using the logistic function.
    ///
    /// # Arguments
    ///
    /// * `time_since_start`: Time since the auction started.
    /// * `sold`: Quantity sold.
    ///
    /// # Returns
    ///
    /// * A `Fixed` representing the price.
    fn get_vrgda_price(self: @LogisticVRGDA, time_since_start: Fixed, sold: Fixed) -> Fixed {
        *self.target_price
            * exp(
                *self.decay_constant
                    * (time_since_start
                        - self.get_target_sale_time(sold + FixedTrait::new(1, false)))
            )
    }
}

