const TOLERANCE: u128 = 18446744073709550; // 0.001

fn assert_approx_equal(expected: u128, actual: u128, tolerance: u128) {
    let left_bound = expected - tolerance;
    let right_bound = expected + tolerance;
    assert(left_bound <= actual && actual <= right_bound, 'Not approx eq');
}
