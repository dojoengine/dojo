use cubit::types::fixed::{Fixed, FixedMul, FixedSub, FixedDiv};

#[derive(Component, Drop)]
struct GradualDutchAuction {
    initial_price: Fixed,
    scale_factor: Fixed,
    decay_constant: Fixed,
    auction_start_time: Fixed,
}

trait GradualDutchAuctionTrait {
    fn purchase_price(self: @Market, quantity: u128, existing: u128, current_time: u128) -> Fixed;
}

impl GradualDutchAuctionImpl of GradualDutchAuctionTrait {
    fn purchase_price(self: @Market, quantity: u128, existing: u128, current_time: u128) -> Fixed {
        let quantity_fp = Fixed::new(quantity, true);
        let existing_fp = Fixed::new(existing, true);
        let current_time_fp = Fixed::new(current_time, true);

        let num1_pow = Fixed::pow(*self.scale_factor, existing_fp);
        let num1 = FixedMul::mul(*self.initial_price, num1_pow);

        let num2_pow = Fixed::pow(*self.scale_factor, quantity_fp);
        let num2 = FixedMul::mul(num2_pow, Fixed::new(1, true));

        let den1_mul = FixedMul::mul(*self.decay_constant, *self.auction_start_time);
        let den1 = Fixed::exp(den1_mul);
        let den2 = FixedSub::sub(*self.scale_factor, Fixed::new(1, true));

        let mul_num2 = FixedMul::mul(num1, num2);
        let mul_num3 = FixedMul::mul(den1, den2);

        let total_cost = FixedDiv::div(mul_num2, mul_num3);
        total_cost
    }
}

