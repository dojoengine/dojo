#[system]
mod Buy {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::integer::{u250, ContractAddressIntoU250};
    use dojo_defi::constant_product_market::components::{Item, Cash, Market, MarketTrait};

    fn execute(partition: u250, item_id: u250, quantity: usize) {
        let player: u250 = starknet::get_caller_address().into();

        let cash_sk: Query = (partition, (player)).into_partitioned();
        let player_cash = commands::<Cash>::entity(cash_sk);

        let market_sk: Query = (partition, (item_id)).into_partitioned();
        let market = commands::<Market>::entity(market_sk);

        let cost = market.buy(quantity);
        assert(cost <= player_cash.amount, 'not enough cash');

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
        let item_sk: Query = (partition, (player, item_id)).into_partitioned();
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
    use dojo_core::integer::{u250, ContractAddressIntoU250};
    use dojo_defi::constant_product_market::components::{Item, Cash, Market, MarketTrait};

    fn execute(partition: u250, item_id: u250, quantity: usize) {
        let player: u250 = starknet::get_caller_address().into();

        let item_sk: Query = (partition, (player, item_id)).into_partitioned();
        let maybe_item = commands::<Item>::try_entity(item_sk);
        let player_quantity = match maybe_item {
            Option::Some(item) => item.quantity,
            Option::None(_) => 0,
        };
        assert(player_quantity >= quantity, 'not enough items');

        let cash_sk: Query = (partition, (player)).into_partitioned();
        let player_cash = commands::<Cash>::entity(cash_sk);

        let market_sk: Query = (partition, (item_id)).into_partitioned();
        let market = commands::<Market>::entity(market_sk);
        let payout = market.sell(quantity);

        // update market
        commands::set_entity(
            market_sk,
            (Market {
                cash_amount: market.cash_amount - payout,
                item_quantity: market.item_quantity + quantity,
            })
        );

        // update player cash
        commands::set_entity(cash_sk, (Cash { amount: player_cash.amount + payout }));

        // update player item
        commands::set_entity(item_sk, (Item { quantity: player_quantity - quantity }));
    }
}

#[system]
mod AddLiquidity {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::integer::{u250, ContractAddressIntoU250};
    use dojo_defi::constant_product_market::components::{Item, Cash, Market, Liquidity, MarketTrait};

    use cubit::types::fixed::FixedType;

    fn execute(partition: u250, item_id: u250, amount: u128, quantity: usize) {
        let player: u250 = starknet::get_caller_address().into();

        let item_sk: Query = (partition, (player, item_id)).into_partitioned();
        let maybe_item = commands::<Item>::try_entity(item_sk);
        let player_quantity = match maybe_item {
            Option::Some(item) => item.quantity,
            Option::None(_) => 0,
        };
        assert(player_quantity >= quantity, 'not enough items');

        let cash_sk: Query = (partition, (player)).into_partitioned();
        let player_cash = commands::<Cash>::entity(cash_sk);
        assert(amount <= player_cash.amount, 'not enough cash');

        let market_sk: Query = (partition, (item_id)).into_partitioned();
        let market = commands::<Market>::entity(market_sk);
        let (cost_cash, cost_quantity, liquidity_shares) = market.add_liquidity(amount, quantity);

        // update market
        commands::set_entity(
            market_sk,
            (Market {
                cash_amount: market.cash_amount + cost_cash,
                item_quantity: market.item_quantity + cost_quantity
            })
        );

        // update player cash
        commands::set_entity(cash_sk, (Cash { amount: player_cash.amount - cost_cash }));

        // update player item
        commands::set_entity(item_sk, (Item { quantity: player_quantity - cost_quantity }));

        // update player liquidity
        let liquidity_sk: Query = (partition, (player, item_id)).into_partitioned();
        let player_liquidity = commands::<Liquidity>::entity(liquidity_sk);
        commands::set_entity(
            liquidity_sk, (Liquidity { shares: player_liquidity.shares + liquidity_shares })
        );
    }
}

#[system]
mod RemoveLiquidity {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_core::integer::{u250, ContractAddressIntoU250};
    use dojo_defi::constant_product_market::components::{Item, Cash, Market, Liquidity, MarketTrait};

    use cubit::types::fixed::FixedType;
    use serde::Serde;

    fn execute(partition: u250, item_id: u250, shares: FixedType) {
        let player: u250 = starknet::get_caller_address().into();

        let liquidity_sk: Query = (partition, (player, item_id)).into_partitioned();
        let player_liquidity = commands::<Liquidity>::entity(liquidity_sk);
        assert(player_liquidity.shares >= shares, 'not enough shares');

        let market_sk: Query = (partition, (item_id)).into_partitioned();
        let market = commands::<Market>::entity(market_sk);
        let (payout_cash, payout_quantity) = market.remove_liquidity(shares);

        // update market
        commands::set_entity(
            market_sk,
            (Market {
                cash_amount: market.cash_amount - payout_cash,
                item_quantity: market.item_quantity - payout_quantity
            })
        );

        // update player cash
        let cash_sk: Query = (partition, (player)).into_partitioned();
        let player_cash = commands::<Cash>::entity(cash_sk);
        commands::set_entity(cash_sk, (Cash { amount: player_cash.amount + payout_cash }));

        // update player item
        let item_sk: Query = (partition, (player, item_id)).into_partitioned();
        let maybe_item = commands::<Item>::try_entity(item_sk);
        let player_quantity = match maybe_item {
            Option::Some(item) => item.quantity,
            Option::None(_) => 0,
        };
        commands::set_entity(item_sk, (Item { quantity: player_quantity + payout_quantity }));

        // update player liquidity
        let liquidity_sk: Query = (partition, (player, item_id)).into_partitioned();
        let player_liquidity = commands::<Liquidity>::entity(liquidity_sk);
        commands::set_entity(
            liquidity_sk, (Liquidity { shares: player_liquidity.shares - shares })
        );
    }
}
