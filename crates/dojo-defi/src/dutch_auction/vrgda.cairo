use cubit::types::fixed::{Fixed, FixedType, FixedAdd, FixedMul, FixedSub, FixedDiv};

#[derive(Component)]
#[derive(Drop)]
struct Market {
    target_price: FixedType,
    scale_factor: FixedType,
    decay_constant: FixedType,
    per_time_unit: FixedType,
}

trait VrgdaTrait {
    fn get_target_sale_time(sold: u128) -> FixedType;
    fn vrgda_price(time_start: u128, sold: u128) -> FixedType;
}

impl VrgdaImpl of VrgdaTrait {
    fn get_target_sale_time(self: @Market, sold: u128) -> FixedType {
        let sold_fp = Fixed::new(sold, false);

        FixedDiv::div(sold_fp, *self.per_time_unit)
    }

    fn vrgda_price(time_start: u128, sold: u128) -> FixedType {
        time_start_fp = Fixed::new(time_start, false);
        sold_fp = Fixed::new(sold, false);

        num1 = FixedAdd::add(sold_fp, Fixed::new(1, false));
        num2 = FixedSub::sub(time_start_fp, num1);
        num3 = FixedMul::mul(*self.decay_constant, num2);
        num4 = Fixed::exp(num3);

        FixedMul::mul(*self.target_price, num4)
    }
}
