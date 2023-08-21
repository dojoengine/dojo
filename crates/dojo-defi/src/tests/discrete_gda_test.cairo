use starknet::testing::set_block_timestamp;

use cubit::f128::types::fixed::{Fixed, FixedTrait, ONE_u128};
use cubit::f128::math::core::div;

use dojo_defi::dutch_auction::gda::{DiscreteGDA, DiscreteGDATrait};
use dojo_defi::tests::utils::{assert_approx_equal, TOLERANCE};

#[test]
#[available_gas(2000000)]
fn test_initial_price() {
    let auction = DiscreteGDA {
        sold: FixedTrait::new_unscaled(0, false),
        initial_price: FixedTrait::new_unscaled(1000, false),
        scale_factor: div(FixedTrait::new_unscaled(11, false), FixedTrait::new_unscaled(10, false)),
        decay_constant: div(FixedTrait::new_unscaled(1, false), FixedTrait::new_unscaled(2, false)),
        auction_start_time: 0,
    };
    let price: Fixed = auction.purchase_price(1);
    assert(price == auction.initial_price, 'wrong price')
}

// ipynb with calculations at https://colab.research.google.com/drive/14elIFRXdG3_gyiI43tP47lUC_aClDHfB?usp=sharing
#[test]
#[available_gas(2000000)]
fn test_price() {
    let auction = DiscreteGDA {
        sold: FixedTrait::new_unscaled(2, false),
        initial_price: FixedTrait::new_unscaled(1000, false),
        scale_factor: div(FixedTrait::new_unscaled(11, false), FixedTrait::new_unscaled(10, false)),
        decay_constant: div(FixedTrait::new_unscaled(1, false), FixedTrait::new_unscaled(2, false)),
        auction_start_time: 0,
    };
    let expected = FixedTrait::new(2396905028162956000000, false);
    set_block_timestamp(10);
    let price: Fixed = auction.purchase_price(10);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}
