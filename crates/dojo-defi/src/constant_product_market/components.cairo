use traits::Into;
use traits::TryInto;
use option::OptionTrait;

const SCALING_FACTOR: u128 = 10000_u128;

#[derive(Component)]
struct Cash {
    amount: u128, 
}

#[derive(Component)]
struct Item {
    quantity: usize,
}

#[derive(Component)]
struct Market {
    cash_amount: u128,
    item_quantity: usize,
}

trait MarketTrait {
    fn buy(self: @Market, quantity: usize) -> u128;
    fn sell(self: @Market, quantity: usize) -> u128;
}

impl MarketImpl of MarketTrait {
    fn buy(self: @Market, quantity: usize) -> u128 {
        assert(quantity < *self.item_quantity, 'not enough liquidity');
        let (quantity, available, cash) = normalize(quantity, self);
        let k = cash * available;
        let cost = (k / (available - quantity)) - cash;
        cost
    }

    fn sell(self: @Market, quantity: usize) -> u128 {
        let (quantity, available, cash) = normalize(quantity, self);
        let k = cash * available;
        let payout = cash - (k / (available + quantity));
        payout
    }
}

fn normalize(quantity: usize, market: @Market) -> (u128, u128, u128) {
    let quantity: u128 = quantity.into().try_into().unwrap() * SCALING_FACTOR;
    let available: u128 = (*market.item_quantity).into().try_into().unwrap() * SCALING_FACTOR;
    (quantity, available, *market.cash_amount)
}


#[test]
#[should_panic(expected: ('not enough liquidity', ))]
fn test_not_enough_quantity() {
    let market = Market {
        cash_amount: SCALING_FACTOR * 1_u128, item_quantity: 1_usize
    }; // pool 1:1
    let cost = market.buy(10_usize);
}

#[test]
#[available_gas(100000)]
fn test_market_buy() {
    let market = Market {
        cash_amount: SCALING_FACTOR * 1_u128, item_quantity: 10_usize
    }; // pool 1:10
    let cost = market.buy(5_usize);
    assert(cost == SCALING_FACTOR * 1_u128, 'wrong cost');
}

#[test]
#[available_gas(100000)]
fn test_market_sell() {
    let market = Market {
        cash_amount: SCALING_FACTOR * 1_u128, item_quantity: 10_usize
    }; // pool 1:10
    let payout = market.sell(5_usize);
    assert(payout == 3334_u128, 'wrong payout');
}
