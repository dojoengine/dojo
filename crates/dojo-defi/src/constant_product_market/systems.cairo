#[system]
mod Buy {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_defi::constant_product_market::components::Item;
    use dojo_defi::constant_product_market::components::Cash;
    use dojo_defi::constant_product_market::components::Market;
    use dojo_defi::constant_product_market::components::MarketTrait;

    fn execute(game_id: felt252, item_id: felt252, quantity: usize) {
        let player: felt252 = starknet::get_caller_address().into();

        let cash_sk: Query = (game_id, (player)).into();
        let player_cash = commands::<Cash>::entity(cash_sk);

        let market_sk: Query = (game_id, (item_id)).into();
        let market = commands::<Market>::entity(market_sk);

        let cost = market.buy(quantity);
        assert(cost < player_cash.amount, 'not enough cash');

        // update market
        commands::set_entity(
            market_sk,
            (Market {
                cash_amount: market.cash_amount + cost,
                item_quantity: market.item_quantity - quantity,
            })
        );

        // update player cash
        commands::set_entity(cash_sk, (Cash { amount: player_cash.amount - cost }));

        // update player item
        let item_sk: Query = (game_id, (player, item_id)).into();
        let maybe_item = commands::<Item>::try_entity(item_sk);
        let player_quantity = match maybe_item {
            Option::Some(item) => item.quantity + quantity,
            Option::None(_) => quantity,
        };
        commands::set_entity(item_sk, (Item { quantity: player_quantity }));
    }
}

#[system]
mod Sell {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_defi::constant_product_market::components::Item;
    use dojo_defi::constant_product_market::components::Cash;
    use dojo_defi::constant_product_market::components::Market;
    use dojo_defi::constant_product_market::components::MarketTrait;

    fn execute(game_id: felt252, item_id: felt252, quantity: usize) {
        let player: felt252 = starknet::get_caller_address().into();

        let item_sk: Query = (game_id, (player, item_id)).into();
        let maybe_item = commands::<Item>::try_entity(item_sk);
        let player_quantity = match maybe_item {
            Option::Some(item) => item.quantity,
            Option::None(_) => 0_u32,
        };
        assert(player_quantity >= quantity, 'not enough items');

        let cash_sk: Query = (game_id, (player)).into();
        let player_cash = commands::<Cash>::entity(cash_sk);

        let market_sk: Query = (game_id, (item_id)).into();
        let market = commands::<Market>::entity(market_sk);
        let payout = market.sell(quantity);

        // update market
        commands::set_entity(
            market_sk,
            (Market {
                cash_amount: market.cash_amount - payout,
                item_quantity: market.item_quantity + quantity
            })
        );

        // update player cash
        commands::set_entity(cash_sk, (Cash { amount: player_cash.amount + payout }));

        // update player item
        commands::set_entity(item_sk, (Item { quantity: player_quantity - quantity }));
    }
}