use starknet::get_block_timestamp;
use traits::Into;

use cubit::f128::types::fixed::{Fixed, FixedTrait, ONE_u128};
use cubit::f128::math::core::{mul, sub, div, exp, pow};

#[derive(Copy, Drop, Serde, starknet::Storage)]
struct DiscreteGDA {
    sold: Fixed,
    initial_price: Fixed,
    scale_factor: Fixed,
    decay_constant: Fixed,
    auction_start_time: u64,
}

trait DiscreteGDATrait<Fixed> {
    fn purchase_price(self: @DiscreteGDA, quantity: u128, ) -> Fixed;
}


impl DiscreteGDAImpl of DiscreteGDATrait<Fixed> {
    fn purchase_price(self: @DiscreteGDA, quantity: u128) -> Fixed {
        let quantity_fp = FixedTrait::new_unscaled(quantity, false);
        let time_since_start = get_block_timestamp().into() - *self.auction_start_time;
        let time_since_start_fp = FixedTrait::new_unscaled(time_since_start.into(), false);

        let num1 = mul(*self.initial_price, pow(*self.scale_factor, *self.sold));
        let num2 = sub(pow(*self.scale_factor, quantity_fp), FixedTrait::ONE());
        let den1 = exp(mul(*self.decay_constant, time_since_start_fp));
        let den2 = sub(*self.scale_factor, FixedTrait::ONE());
        div(mul(num1, num2), mul(den1, den2))
    }
}

#[derive(Copy, Drop, Serde, starknet::Storage)]
struct ContinuousGDA {
    initial_price: Fixed,
    emission_rate: Fixed,
    decay_constant: Fixed,
    last_auction_start: u64,
}

trait ContinuousGDATrait<Fixed> {
    fn purchase_price(self: @ContinuousGDA, quantity: u128) -> Fixed;
}

impl ContinuousGDAImpl of ContinuousGDATrait<Fixed> {
    fn purchase_price(self: @ContinuousGDA, quantity: u128) -> Fixed {
        let quantity_fp = FixedTrait::new_unscaled(quantity, false);
        let time_since_last_auction = get_block_timestamp() - *self.last_auction_start;
        let time_since_last_auction_fp = FixedTrait::new_unscaled(
            time_since_last_auction.into(), false
        );
        let num1 = div(*self.initial_price, *self.decay_constant);
        let num2 = sub(
            exp(div(mul(*self.decay_constant, quantity_fp), *self.emission_rate)), FixedTrait::ONE()
        );
        let den = exp(mul(*self.decay_constant, time_since_last_auction_fp));
        div(mul(num1, num2), den)
    }
}
