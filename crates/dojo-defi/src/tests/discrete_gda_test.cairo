use cubit::f128::types::fixed::{Fixed, FixedTrait};

use dojo_defi::dutch_auction::gda::{DiscreteGDA, DiscreteGDATrait};
use dojo_defi::tests::utils::{assert_approx_equal, TOLERANCE};

#[test]
#[available_gas(2000000)]
fn test_initial_price() {
    let auction = DiscreteGDA {
        sold: FixedTrait::new_unscaled(0, false),
        initial_price: FixedTrait::new_unscaled(1000, false),
        scale_factor: FixedTrait::new_unscaled(11, false) / FixedTrait::new_unscaled(10, false),
        decay_constant: FixedTrait::new_unscaled(1, false) / FixedTrait::new_unscaled(2, false),
    };
    let price = auction.purchase_price(FixedTrait::ZERO(), FixedTrait::ONE());
    assert_approx_equal(price.mag, auction.initial_price.mag, TOLERANCE)
}

// ipynb with calculations at https://colab.research.google.com/drive/14elIFRXdG3_gyiI43tP47lUC_aClDHfB?usp=sharing
#[test]
#[available_gas(2000000)]
fn test_price() {
    let auction = DiscreteGDA {
        sold: FixedTrait::new_unscaled(2, false),
        initial_price: FixedTrait::new_unscaled(1000, false),
        scale_factor: FixedTrait::new_unscaled(11, false) / FixedTrait::new_unscaled(10, false),
        decay_constant: FixedTrait::new_unscaled(1, false) / FixedTrait::new_unscaled(2, false),
    };
    let expected = FixedTrait::new(2396905028162956000000, false);
    let price = auction
        .purchase_price(FixedTrait::new_unscaled(10, false), FixedTrait::new_unscaled(10, false), );
    assert_approx_equal(price.mag, expected.mag, TOLERANCE)
}
