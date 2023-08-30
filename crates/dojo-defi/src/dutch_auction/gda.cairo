use cubit::f128::math::core::{exp, pow};
use cubit::f128::types::fixed::{Fixed, FixedTrait};

use debug::PrintTrait;

/// A Gradual Dutch Auction represented using discrete time steps.
/// The purchase price for a given quantity is calculated based on
/// the initial price, scale factor, decay constant, and the time since
/// the auction has started.
#[derive(Copy, Drop, Serde, starknet::Storage)]
struct DiscreteGDA {
    sold: Fixed,
    initial_price: Fixed,
    scale_factor: Fixed,
    decay_constant: Fixed,
}

#[generate_trait]
impl DiscreteGDAImpl of DiscreteGDATrait {
    /// Calculates the purchase price for a given quantity of the item at a specific time.
    ///
    /// # Arguments
    ///
    /// * `time_since_start`: Time since the start of the auction in days.
    /// * `quantity`: Quantity of the item being purchased.
    ///
    /// # Returns
    ///
    /// * A `Fixed` representing the purchase price.
    fn purchase_price(self: @DiscreteGDA, time_since_start: Fixed, quantity: Fixed) -> Fixed {
        let num1 = *self.initial_price * pow(*self.scale_factor, *self.sold);
        let num2 = pow(*self.scale_factor, quantity) - FixedTrait::ONE();
        let den1 = exp(*self.decay_constant * time_since_start);
        let den2 = *self.scale_factor - FixedTrait::ONE();
        (num1 * num2) / (den1 * den2)
    }
}

/// A Gradual Dutch Auction represented using continuous time steps.
/// The purchase price is calculated based on the initial price,
/// emission rate, decay constant, and the time since the last purchase in days.
#[derive(Copy, Drop, Serde, starknet::Storage)]
struct ContinuousGDA {
    initial_price: Fixed,
    emission_rate: Fixed,
    decay_constant: Fixed,
}

#[generate_trait]
impl ContinuousGDAImpl of ContinuousGDATrait {
    /// Calculates the purchase price for a given quantity of the item at a specific time.
    ///
    /// # Arguments
    ///
    /// * `time_since_last`: Time since the last purchase in the auction in days.
    /// * `quantity`: Quantity of the item being purchased.
    ///
    /// # Returns
    ///
    /// * A `Fixed` representing the purchase price.
    fn purchase_price(self: @ContinuousGDA, time_since_last: Fixed, quantity: Fixed) -> Fixed {
        let num1 = *self.initial_price / *self.decay_constant;
        let num2 = exp((*self.decay_constant * quantity) / *self.emission_rate) - FixedTrait::ONE();
        let den = exp(*self.decay_constant * time_since_last);
        (num1 * num2) / den
    }
}
