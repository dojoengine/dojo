use cubit::f128::types::fixed::{Fixed, FixedTrait};

use dojo_defi::dutch_auction::common::{from_days_fp};
use dojo_defi::dutch_auction::vrgda::{LogisticVRGDA, LogisticVRGDATrait};
use dojo_defi::tests::utils::assert_rel_approx_eq;


use debug::PrintTrait;
const _69_42: u128 = 1280572973596917000000;
const _0_31: u128 = 5718490662849961000;
const DELTA_0_0005: u128 = 9223372036854776;
const DELTA_0_02: u128 = 368934881474191000;
const MAX_SELLABLE: u128 = 6392;
const _0_0023: u128 = 42427511369531970;

#[test]
#[available_gas(200000000)]
fn test_target_price() {
    let auction = LogisticVRGDA {
        target_price: FixedTrait::new(_69_42, false),
        decay_constant: FixedTrait::new(_0_31, false),
        max_sellable: FixedTrait::new_unscaled(MAX_SELLABLE, false),
        time_scale: FixedTrait::new(_0_0023, false),
    };
    let time = from_days_fp(auction.get_target_sale_time(FixedTrait::new(1, false)));

    let cost = auction.get_vrgda_price(time + FixedTrait::new(1, false), FixedTrait::ZERO());
    assert_rel_approx_eq(cost, auction.target_price, FixedTrait::new(DELTA_0_0005, false));
}

#[test]
#[available_gas(200000000)]
fn test_pricing_basic() {
    let auction = LogisticVRGDA {
        target_price: FixedTrait::new(_69_42, false),
        decay_constant: FixedTrait::new(_0_31, false),
        max_sellable: FixedTrait::new_unscaled(MAX_SELLABLE, false),
        time_scale: FixedTrait::new(_0_0023, false),
    };
    let time_delta = FixedTrait::new(10368001, false);
    let num_mint = FixedTrait::new(876, false);

    let cost = auction.get_vrgda_price(time_delta, num_mint);
    assert_rel_approx_eq(cost, auction.target_price, FixedTrait::new(DELTA_0_02, false));
}

#[test]
#[available_gas(200000000)]
fn test_pricing_basic_reverse() {
    let auction = LogisticVRGDA {
        target_price: FixedTrait::new(_69_42, false),
        decay_constant: FixedTrait::new(_0_31, false),
        max_sellable: FixedTrait::new_unscaled(MAX_SELLABLE, false),
        time_scale: FixedTrait::new(_0_0023, false),
    };
    let time_delta = FixedTrait::new(10368001, false);
    let num_mint = FixedTrait::new(876, false);

    let cost = auction.get_reverse_vrgda_price(time_delta, num_mint);
    assert_rel_approx_eq(cost, auction.target_price, FixedTrait::new(DELTA_0_02, false));
}

