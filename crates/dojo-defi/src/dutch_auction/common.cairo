use cubit::f128::types::fixed::{Fixed, FixedTrait, ONE_u128};

fn to_days_fp(x: Fixed) -> Fixed {
    x / FixedTrait::new(86400, false)
}

fn from_days_fp(x: Fixed) -> Fixed {
    x * FixedTrait::new(86400, false)
}
