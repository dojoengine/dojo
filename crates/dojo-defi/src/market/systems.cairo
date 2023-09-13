#[system]
mod Buy {
    use dojo_defi::market::components::{Item, Cash, Market};
    use dojo_defi::market::constant_product_market::MarketTrait;
    use dojo::world::Context;

    fn execute(ctx: Context, item_id: u32, quantity: u128) {
        let player = starknet::get_caller_address();

        let player_cash = get!(ctx.world, (player), Cash);

        let market = get!(ctx.world, (item_id), Market);

        let cost = market.buy(quantity);
        assert(cost <= player_cash.amount, 'not enough cash');

        // update market
        set!(
            ctx.world,
            (Market {
                item_id: item_id,
                cash_amount: market.cash_amount + cost,
                item_quantity: market.item_quantity - quantity,
            })
        );

        // update player cash
        set!(ctx.world, (Cash { player: player, amount: player_cash.amount - cost }));

        // update player item
        let item = get!(ctx.world, (player, item_id), Item);
        set!(
            ctx.world,
            (Item { player: player, item_id: item_id, quantity: item.quantity + quantity })
        );
    }
}

#[system]
mod Sell {
    use dojo_defi::market::components::{Item, Cash, Market};
    use dojo_defi::market::constant_product_market::MarketTrait;
    use dojo::world::Context;

    fn execute(ctx: Context, item_id: u32, quantity: u128) {
        let player = starknet::get_caller_address();

        let item = get!(ctx.world, (player, item_id), Item);
        let player_quantity = item.quantity;
        assert(player_quantity >= quantity, 'not enough items');

        let player_cash = get!(ctx.world, (player), Cash);

        let market = get!(ctx.world, (item_id), Market);
        let payout = market.sell(quantity);

        // update market
        set!(
            ctx.world,
            (Market {
                item_id: item_id,
                cash_amount: market.cash_amount - payout,
                item_quantity: market.item_quantity + quantity,
            })
        );

        // update player cash
        set!(ctx.world, (Cash { player: player, amount: player_cash.amount + payout }));

        // update player item
        set!(
            ctx.world,
            (Item { player: player, item_id: item_id, quantity: player_quantity - quantity })
        );
    }
}

#[system]
mod AddLiquidity {
    use dojo_defi::market::components::{Item, Cash, Market, Liquidity};
    use dojo_defi::market::constant_product_market::MarketTrait;
    use dojo::world::Context;

    fn execute(ctx: Context, item_id: u32, amount: u128, quantity: u128) {
        let player = starknet::get_caller_address();

        let item = get!(ctx.world, (player, item_id), Item);
        let player_quantity = item.quantity;
        assert(player_quantity >= quantity, 'not enough items');

        let player_cash = get!(ctx.world, (player), Cash);
        assert(amount <= player_cash.amount, 'not enough cash');

        let market = get!(ctx.world, (item_id), Market);
        let (cost_cash, cost_quantity, liquidity_shares) = market.add_liquidity(amount, quantity);

        // update market
        set!(
            ctx.world,
            (Market {
                item_id: item_id,
                cash_amount: market.cash_amount + cost_cash,
                item_quantity: market.item_quantity + cost_quantity
            })
        );

        // update player cash
        set!(ctx.world, (Cash { player: player, amount: player_cash.amount - cost_cash }));

        // update player item
        set!(
            ctx.world,
            (Item { player: player, item_id: item_id, quantity: player_quantity - cost_quantity })
        );

        // update player liquidity
        let player_liquidity = get!(ctx.world, (player, item_id), Liquidity);
        set!(
            ctx.world,
            (Liquidity {
                player: player, item_id: item_id, shares: player_liquidity.shares + liquidity_shares
            })
        );
    }
}

#[system]
mod RemoveLiquidity {
    use dojo_defi::market::components::{Item, Cash, Market, Liquidity};
    use dojo_defi::market::constant_product_market::MarketTrait;
    use dojo::world::Context;

    use cubit::f128::types::fixed::Fixed;

    fn execute(ctx: Context, item_id: u32, shares: Fixed) {
        let player = starknet::get_caller_address();

        let player_liquidity = get!(ctx.world, (player, item_id), Liquidity);
        assert(player_liquidity.shares >= shares, 'not enough shares');

        let market = get!(ctx.world, (item_id), Market);
        let (payout_cash, payout_quantity) = market.remove_liquidity(shares);

        // update market
        set!(
            ctx.world,
            (Market {
                item_id: item_id,
                cash_amount: market.cash_amount - payout_cash,
                item_quantity: market.item_quantity - payout_quantity
            })
        );

        // update player cash
        let player_cash = get!(ctx.world, (player), Cash);
        set!(ctx.world, (Cash { player: player, amount: player_cash.amount + payout_cash }));

        // update player item
        let item = get!(ctx.world, (player, item_id), Item);
        let player_quantity = item.quantity;
        set!(
            ctx.world,
            (Item { player: player, item_id: item_id, quantity: player_quantity + payout_quantity })
        );

        // update player liquidity
        let player_liquidity = get!(ctx.world, (player, item_id), Liquidity);
        set!(
            ctx.world,
            (Liquidity {
                player: player, item_id: item_id, shares: player_liquidity.shares - shares
            })
        );
    }
}

