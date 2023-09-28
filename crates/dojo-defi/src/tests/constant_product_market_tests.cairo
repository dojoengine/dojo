use traits::Into;

use dojo_defi::market::models::Market;
use dojo_defi::market::constant_product_market::{MarketTrait, SCALING_FACTOR};
use dojo_defi::tests::utils::{TOLERANCE, assert_approx_equal};

use cubit::f128::types::FixedTrait;

#[test]
#[should_panic(expected: ('not enough liquidity',))]
fn test_not_enough_quantity() {
    let market = Market {
        item_id: 1, cash_amount: SCALING_FACTOR * 1, item_quantity: 1
    }; // pool 1:1
    let cost = market.buy(10);
}

#[test]
#[available_gas(100000)]
fn test_market_buy() {
    let market = Market {
        item_id: 1, cash_amount: SCALING_FACTOR * 1, item_quantity: 10
    }; // pool 1:10
    let cost = market.buy(5);
    assert(cost == SCALING_FACTOR * 1, 'wrong cost');
}

#[test]
#[available_gas(100000)]
fn test_market_sell() {
    let market = Market {
        item_id: 1, cash_amount: SCALING_FACTOR * 1, item_quantity: 10
    }; // pool 1:10
    let payout = market.sell(5);
    assert(payout == 3334, 'wrong payout');
}

#[test]
#[available_gas(500000)]
fn test_market_add_liquidity_no_initial() {
    // Without initial liquidity
    let market = Market { item_id: 1, cash_amount: 0, item_quantity: 0 };

    // Add liquidity
    let (amount, quantity) = (SCALING_FACTOR * 5, 5); // pool 1:1
    let (amount_add, quantity_add, liquidity_add) = market.add_liquidity(amount, quantity);

    // Assert that the amount and quantity added are the same as the given amount and quantity
    // and that the liquidity shares minted are the same as the entire liquidity
    assert(amount_add == amount, 'wrong cash amount');
    assert(quantity_add == quantity, 'wrong item quantity');

    // Convert amount and quantity to fixed point
    let amount = FixedTrait::new_unscaled(amount, false);
    let quantity: u128 = quantity.into() * SCALING_FACTOR;
    let quantity = FixedTrait::new_unscaled(quantity, false);
    assert(liquidity_add == (amount * quantity).sqrt(), 'wrong liquidity');
}

#[test]
#[available_gas(600000)]
fn test_market_add_liquidity_optimal() {
    // With initial liquidity
    let market = Market {
        item_id: 1, cash_amount: SCALING_FACTOR * 1, item_quantity: 10
    }; // pool 1:10
    let initial_liquidity = market.liquidity();

    // Add liquidity with the same ratio
    let (amount, quantity) = (SCALING_FACTOR * 2, 20); // pool 1:10
    let (amount_add, quantity_add, liquidity_add) = market.add_liquidity(amount, quantity);

    // Assert 
    assert(amount_add == amount, 'wrong cash amount');
    assert(quantity_add == quantity, 'wrong item quantity');

    // Get expected amount and convert to fixed point
    let expected_amount = FixedTrait::new_unscaled(SCALING_FACTOR * 1 + amount, false);
    let expected_quantity: u128 = (10 + quantity).into() * SCALING_FACTOR;
    let expected_quantity = FixedTrait::new_unscaled(expected_quantity, false);

    // Compute the expected liquidity shares
    let expected_liquidity = FixedTrait::sqrt(expected_amount * expected_quantity);
    let final_liquidity = initial_liquidity + liquidity_add;
    assert_approx_equal(expected_liquidity, final_liquidity, TOLERANCE);
}

#[test]
#[available_gas(1000000)]
fn test_market_add_liquidity_not_optimal() {
    // With initial liquidity
    let market = Market {
        item_id: 1, cash_amount: SCALING_FACTOR * 1, item_quantity: 10
    }; // pool 1:10
    let initial_liquidity = market.liquidity();

    // Add liquidity without the same ratio
    let (amount, quantity) = (SCALING_FACTOR * 2, 10); // pool 1:5

    let (amount_add, quantity_add, liquidity_add) = market.add_liquidity(amount, quantity);

    // Assert that the amount added is optimal even though the
    // amount originally requested was not
    let amount_optimal = SCALING_FACTOR * 1;
    assert(amount_add == amount_optimal, 'wrong cash amount');
    assert(quantity_add == quantity, 'wrong item quantity');

    // Get expected amount and convert to fixed point
    let expected_amount = FixedTrait::new_unscaled(SCALING_FACTOR * 1 + amount_add, false);
    let expected_quantity: u128 = (10 + quantity).into() * SCALING_FACTOR;
    let expected_quantity = FixedTrait::new_unscaled(expected_quantity, false);

    // Get expecteed liquidity
    let expected_liquidity = FixedTrait::sqrt(expected_amount * expected_quantity);

    let final_liquidity = initial_liquidity + liquidity_add;
// assert_precise(expected_liquidity, final_liquidity.into(), 'wrong liquidity', Option::None(()));
}

#[test]
#[should_panic(expected: ('insufficient amount',))]
fn test_market_add_liquidity_insufficient_amount() {
    let market = Market {
        item_id: 1, cash_amount: SCALING_FACTOR * 1, item_quantity: 10
    }; // pool 1:10
    // Adding 20 items requires (SCALING_FACTOR * 2) cash amount to maintain the ratio
    // Therefore this should fail
    let (amount_add, quantity_add, liquidity_add) = market.add_liquidity(SCALING_FACTOR * 1, 20);
}

#[test]
#[available_gas(1000000)]
fn test_market_remove_liquidity() {
    // With initial liquidity
    let market = Market {
        item_id: 1, cash_amount: SCALING_FACTOR * 2, item_quantity: 20
    }; // pool 1:10
    let initial_liquidity = market.liquidity();

    // Remove half of the liquidity
    let two = FixedTrait::new_unscaled(2, false);
    let liquidity_remove = initial_liquidity / two;

    let (amount_remove, quantity_remove) = market.remove_liquidity(liquidity_remove);

    // Assert that the amount and quantity removed are half of the initial amount and quantity
    assert(amount_remove == SCALING_FACTOR * 1, 'wrong cash amount');
    assert(quantity_remove == 10, 'wrong item quantity');

    // Get expected amount and convert to fixed point
    let expected_amount = FixedTrait::new_unscaled(SCALING_FACTOR * 2 - amount_remove, false);
    let expected_quantity: u128 = (20 - quantity_remove).into() * SCALING_FACTOR;
    let expected_quantity = FixedTrait::new_unscaled(expected_quantity, false);

    // Get expecteed liquidity
    let expected_liquidity = FixedTrait::sqrt(expected_amount * expected_quantity);

    let final_liquidity = initial_liquidity - liquidity_remove;
// assert_precise(expected_liquidity, final_liquidity.into(), 'wrong liquidity', Option::None(()));
}

#[test]
#[should_panic(expected: ('insufficient liquidity',))]
fn test_market_remove_liquidity_no_initial() {
    // Without initial liquidity
    let market = Market { item_id: 1, cash_amount: 0, item_quantity: 0 }; // pool 1:10

    // Remove liquidity
    let one = FixedTrait::new_unscaled(1, false);

    let (amount_remove, quantity_remove) = market.remove_liquidity(one);
}

#[test]
#[should_panic(expected: ('insufficient liquidity',))]
fn test_market_remove_liquidity_more_than_available() {
    // With initial liquidity
    let market = Market {
        item_id: 1, cash_amount: SCALING_FACTOR * 2, item_quantity: 20
    }; // pool 1:10
    let initial_liquidity = market.liquidity();

    // Remove twice of the liquidity
    let two = FixedTrait::new_unscaled(2, false);
    let liquidity_remove = initial_liquidity * two;

    let (amount_remove, quantity_remove) = market.remove_liquidity(liquidity_remove);
}
