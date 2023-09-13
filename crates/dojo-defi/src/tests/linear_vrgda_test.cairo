use cubit::f128::types::fixed::{Fixed, FixedTrait};

use dojo_defi::dutch_auction::common::{to_days_fp, from_days_fp};
use dojo_defi::dutch_auction::vrgda::{LinearVRGDA, LinearVRGDATrait};
use dojo_defi::tests::utils::assert_rel_approx_eq;

const _69_42: u128 = 1280572973596917000000;
const _0_31: u128 = 5718490662849961000;
const DELTA_0_0005: u128 = 9223372036854776;
const DELTA_0_02: u128 = 368934881474191000;
const DELTA: u128 = 184467440737095;

#[test]
#[available_gas(2000000)]
fn test_target_price() {
    let auction = LinearVRGDA {
        target_price: FixedTrait::new(_69_42, false),
        decay_constant: FixedTrait::new(_0_31, false),
        per_time_unit: FixedTrait::new_unscaled(2, false),
    };
    let time = from_days_fp(auction.get_target_sale_time(FixedTrait::new(1, false)));
    let cost = auction
        .get_vrgda_price(to_days_fp(time + FixedTrait::new(1, false)), FixedTrait::ZERO());
    assert_rel_approx_eq(cost, auction.target_price, FixedTrait::new(DELTA_0_0005, false));
}

#[test]
#[available_gas(20000000)]
fn test_pricing_basic() {
    let auction = LinearVRGDA {
        target_price: FixedTrait::new(_69_42, false),
        decay_constant: FixedTrait::new(_0_31, false),
        per_time_unit: FixedTrait::new_unscaled(2, false),
    };
    let time_delta = FixedTrait::new(10368001, false); // 120 days
    let num_mint = FixedTrait::new(239, true);
    let cost = auction.get_vrgda_price(time_delta, num_mint);
    assert_rel_approx_eq(cost, auction.target_price, FixedTrait::new(DELTA_0_02, false));
}

#[test]
#[available_gas(20000000)]
fn test_pricing_basic_reverse() {
    let auction = LinearVRGDA {
        target_price: FixedTrait::new(_69_42, false),
        decay_constant: FixedTrait::new(_0_31, false),
        per_time_unit: FixedTrait::new_unscaled(2, false),
    };
    let time_delta = FixedTrait::new(10368001, false); // 120 days
    let num_mint = FixedTrait::new(239, true);
    let cost = auction.get_reverse_vrgda_price(time_delta, num_mint);
    assert_rel_approx_eq(cost, auction.target_price, FixedTrait::new(DELTA_0_02, false));
}

