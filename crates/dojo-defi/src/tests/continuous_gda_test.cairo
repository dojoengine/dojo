use cubit::f128::types::fixed::{Fixed, FixedTrait};

use dojo_defi::dutch_auction::gda::{ContinuousGDA, ContinuousGDATrait};
use dojo_defi::tests::utils::{assert_approx_equal, TOLERANCE};

// ipynb with calculations at https://colab.research.google.com/drive/14elIFRXdG3_gyiI43tP47lUC_aClDHfB?usp=sharing
#[test]
#[available_gas(2000000)]
fn test_price_1() {
    let auction = ContinuousGDA {
        initial_price: FixedTrait::new_unscaled(1000, false),
        emission_rate: FixedTrait::ONE(),
        decay_constant: FixedTrait::new_unscaled(1, false) / FixedTrait::new_unscaled(2, false),
    };
    let expected = FixedTrait::new(22128445337405634000000, false);
    let time_since_last = FixedTrait::new_unscaled(10, false);
    let quantity = FixedTrait::new_unscaled(9, false);
    let price: Fixed = auction.purchase_price(time_since_last, quantity);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}


#[test]
#[available_gas(2000000)]
fn test_price_2() {
    let auction = ContinuousGDA {
        initial_price: FixedTrait::new_unscaled(1000, false),
        emission_rate: FixedTrait::ONE(),
        decay_constant: FixedTrait::new_unscaled(1, false) / FixedTrait::new_unscaled(2, false),
    };
    let expected = FixedTrait::new(89774852279643700000, false);
    let time_since_last = FixedTrait::new_unscaled(20, false);
    let quantity = FixedTrait::new_unscaled(8, false);
    let price: Fixed = auction.purchase_price(time_since_last, quantity);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}

#[test]
#[available_gas(2000000)]
fn test_price_3() {
    let auction = ContinuousGDA {
        initial_price: FixedTrait::new_unscaled(1000, false),
        emission_rate: FixedTrait::ONE(),
        decay_constant: FixedTrait::new_unscaled(1, false) / FixedTrait::new_unscaled(2, false),
    };
    let expected = FixedTrait::new(20393925850936156000, false);
    let time_since_last = FixedTrait::new_unscaled(30, false);
    let quantity = FixedTrait::new_unscaled(15, false);
    let price: Fixed = auction.purchase_price(time_since_last, quantity);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}

#[test]
#[available_gas(2000000)]
fn test_price_4() {
    let auction = ContinuousGDA {
        initial_price: FixedTrait::new_unscaled(1000, false),
        emission_rate: FixedTrait::ONE(),
        decay_constant: FixedTrait::new_unscaled(1, false) / FixedTrait::new_unscaled(2, false),
    };
    let expected = FixedTrait::new(3028401847768577000000, false);
    let time_since_last = FixedTrait::new_unscaled(40, false);
    let quantity = FixedTrait::new_unscaled(35, false);
    let price: Fixed = auction.purchase_price(time_since_last, quantity);
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}

