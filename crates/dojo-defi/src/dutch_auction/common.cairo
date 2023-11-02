use cubit::f128::types::fixed::{Fixed, FixedTrait, ONE_u128};
use dojo_defi::tests::utils::{assert_approx_equal, TOLERANCE};

fn to_days_fp(x: Fixed) -> Fixed {
    x / FixedTrait::new(86400, false)
}

fn from_days_fp(x: Fixed) -> Fixed {
    x * FixedTrait::new(86400, false)
}


#[cfg(test)]
mod test_common {
    #[test]
    #[available_gas(20000000)]
    fn test_days_convertions() {
        let days = FixedTrait::new(2, false);
        assert_approx_equal(days, to_days_fp(from_days_fp(days)), TOLERANCE * 10);
    }
}

