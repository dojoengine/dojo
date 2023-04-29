use traits::Into;
use traits::TryInto;
use option::OptionTrait;

const SCALING_FACTOR: u128 = 10000;

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
        cash_amount: SCALING_FACTOR * 1, item_quantity: 1
    }; // pool 1:1
    let cost = market.buy(10_usize);
}

#[test]
#[available_gas(100000)]
fn test_market_buy() {
    let market = Market {
        cash_amount: SCALING_FACTOR * 1, item_quantity: 10
    }; // pool 1:10
    let cost = market.buy(5);
    assert(cost == SCALING_FACTOR * 1, 'wrong cost');
}

#[test]
#[available_gas(100000)]
fn test_market_sell() {
    let market = Market {
        cash_amount: SCALING_FACTOR * 1, item_quantity: 10
    }; // pool 1:10
    let payout = market.sell(5);
    assert(payout == 3334, 'wrong payout');
}
