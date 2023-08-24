use cubit::f128::types::fixed::{Fixed, FixedTrait};

use debug::PrintTrait;

const TOLERANCE: u128 = 18446744073709550; // 0.001

fn assert_approx_equal(expected: Fixed, actual: Fixed, tolerance: u128) {
    let left_bound = expected - FixedTrait::new(tolerance, false);
    let right_bound = expected + FixedTrait::new(tolerance, false);
    assert(left_bound <= actual && actual <= right_bound, 'Not approx eq');
}

fn assert_rel_approx_eq(a: Fixed, b: Fixed, max_percent_delta: Fixed) {
    if b == FixedTrait::ZERO() {
        assert(a == b, 'a should eq ZERO');
    }
    let percent_delta = if a > b {
        (a - b) / b
    } else {
        (b - a) / b
    };

    assert(percent_delta < max_percent_delta, 'a ~= b not satisfied');
}

