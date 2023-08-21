use starknet::testing::set_block_timestamp;

use cubit::f128::types::fixed::{Fixed, FixedTrait, ONE_u128};
use cubit::f128::math::core::div;

use dojo_defi::dutch_auction::gda::{ContinuousGDA, ContinuousGDATrait};
use dojo_defi::tests::utils::{assert_approx_equal, TOLERANCE};

// ipynb with calculations at https://colab.research.google.com/drive/14elIFRXdG3_gyiI43tP47lUC_aClDHfB?usp=sharing
#[test]
#[available_gas(2000000)]
fn test_price_1() {
    let auction = ContinuousGDA {
        initial_price: FixedTrait::new_unscaled(1000, false),
        emission_rate: FixedTrait::ONE(),
        decay_constant: div(FixedTrait::new_unscaled(1, false), FixedTrait::new_unscaled(2, false)),
        last_auction_start: 0,
    };
    let expected = FixedTrait::new(22128445337405634000000, false);
    set_block_timestamp(10);
    let price: Fixed = auction.purchase_price(9);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}

#[test]
#[available_gas(2000000)]
fn test_price_2() {
    let auction = ContinuousGDA {
        initial_price: FixedTrait::new_unscaled(1000, false),
        emission_rate: FixedTrait::ONE(),
        decay_constant: div(FixedTrait::new_unscaled(1, false), FixedTrait::new_unscaled(2, false)),
        last_auction_start: 0,
    };
    let expected = FixedTrait::new(89774852279643700000, false);
    set_block_timestamp(20);
    let price: Fixed = auction.purchase_price(8);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}

#[test]
#[available_gas(2000000)]
fn test_price_3() {
    let auction = ContinuousGDA {
        initial_price: FixedTrait::new_unscaled(1000, false),
        emission_rate: FixedTrait::ONE(),
        decay_constant: div(FixedTrait::new_unscaled(1, false), FixedTrait::new_unscaled(2, false)),
        last_auction_start: 0,
    };
    let expected = FixedTrait::new(20393925850936156000, false);
    set_block_timestamp(30);
    let price: Fixed = auction.purchase_price(15);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}

#[test]
#[available_gas(2000000)]
fn test_price_4() {
    let auction = ContinuousGDA {
        initial_price: FixedTrait::new_unscaled(1000, false),
        emission_rate: FixedTrait::ONE(),
        decay_constant: div(FixedTrait::new_unscaled(1, false), FixedTrait::new_unscaled(2, false)),
        last_auction_start: 0,
    };
    let expected = FixedTrait::new(3028401847768577000000, false);
    set_block_timestamp(40);
    let price: Fixed = auction.purchase_price(35);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}
