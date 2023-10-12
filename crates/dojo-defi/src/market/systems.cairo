use cubit::f128::types::fixed::Fixed;
use dojo::world::IWorldDispatcher;


trait ITrade<TContractState> {
    fn buy(self: @TContractState, world: IWorldDispatcher, item_id: u32, quantity: u128);
    fn sell(self: @TContractState, world: IWorldDispatcher, item_id: u32, quantity: u128);
}


#[dojo::contract]
mod Trade {
    use dojo_defi::market::models::{Item, Cash, Market};
    use dojo_defi::market::constant_product_market::MarketTrait;

    use super::ITrade;

    
    #[external(v0)]
    impl TradeImpl of ITrade<ContractState> {
        fn buy(self: @ContractState, world: IWorldDispatcher, item_id: u32, quantity: u128) {
            
            let player = starknet::get_caller_address();

            let player_cash = get!(world, (player), Cash);

            let market = get!(world, (item_id), Market);

            let cost = market.buy(quantity);
            assert(cost <= player_cash.amount, 'not enough cash');

            // update market
            set!(
                world,
                (Market {
                    item_id: item_id,
                    cash_amount: market.cash_amount + cost,
                    item_quantity: market.item_quantity - quantity,
                })
            );

            // update player cash
            set!(world, (Cash { player: player, amount: player_cash.amount - cost }));

            // update player item
            let item = get!(world, (player, item_id), Item);
            set!(
                world,
                (Item { player: player, item_id: item_id, quantity: item.quantity + quantity })
            );
        }



        fn sell(self: @ContractState, world: IWorldDispatcher, item_id: u32, quantity: u128) {
            let player = starknet::get_caller_address();

            let item = get!(world, (player, item_id), Item);
            let player_quantity = item.quantity;
            assert(player_quantity >= quantity, 'not enough items');

            let player_cash = get!(world, (player), Cash);

            let market = get!(world, (item_id), Market);
            let payout = market.sell(quantity);

            // update market
            set!(
                world,
                (Market {
                    item_id: item_id,
                    cash_amount: market.cash_amount - payout,
                    item_quantity: market.item_quantity + quantity,
                })
            );

            // update player cash
            set!(world, (Cash { player: player, amount: player_cash.amount + payout }));

            // update player item
            set!(
                world,
                (Item { player: player, item_id: item_id, quantity: player_quantity - quantity })
            );
        }

    }
    
}



trait ILiquidity<TContractState> {
    fn add(self: @TContractState, world: IWorldDispatcher, item_id: u32, amount: u128, quantity: u128);
    fn remove(self: @TContractState, world: IWorldDispatcher, item_id: u32, shares: Fixed);
}


#[dojo::contract]
mod Liquidity {
    use cubit::f128::types::fixed::Fixed;

    use dojo_defi::market::models::{Item, Cash, Market, Liquidity};
    use dojo_defi::market::constant_product_market::MarketTrait;
    
    use super::ILiquidity;

    #[external(v0)]
    impl LiquidityImpl of ILiquidity<ContractState> {
        
        fn add(self: @ContractState, world: IWorldDispatcher, item_id: u32, amount: u128, quantity: u128) {
            let player = starknet::get_caller_address();

            let item = get!(world, (player, item_id), Item);
            let player_quantity = item.quantity;
            assert(player_quantity >= quantity, 'not enough items');

            let player_cash = get!(world, (player), Cash);
            assert(amount <= player_cash.amount, 'not enough cash');

            let market = get!(world, (item_id), Market);
            let (cost_cash, cost_quantity, liquidity_shares) = market.add_liquidity(amount, quantity);

            // update market
            set!(
                world,
                (Market {
                    item_id: item_id,
                    cash_amount: market.cash_amount + cost_cash,
                    item_quantity: market.item_quantity + cost_quantity
                })
            );

            // update player cash
            set!(world, (Cash { player: player, amount: player_cash.amount - cost_cash }));

            // update player item
            set!(
                world,
                (Item { player: player, item_id: item_id, quantity: player_quantity - cost_quantity })
            );

            // update player liquidity
            let player_liquidity = get!(world, (player, item_id), Liquidity);
            set!(
                world,
                (Liquidity {
                    player: player, item_id: item_id, shares: player_liquidity.shares + liquidity_shares
                })
            );
        }


        fn remove(self: @ContractState, world: IWorldDispatcher, item_id: u32, shares: Fixed) {
            let player = starknet::get_caller_address();

            let player_liquidity = get!(world, (player, item_id), Liquidity);
            assert(player_liquidity.shares >= shares, 'not enough shares');

            let market = get!(world, (item_id), Market);
            let (payout_cash, payout_quantity) = market.remove_liquidity(shares);

            // update market
            set!(
                world,
                (Market {
                    item_id: item_id,
                    cash_amount: market.cash_amount - payout_cash,
                    item_quantity: market.item_quantity - payout_quantity
                })
            );

            // update player cash
            let player_cash = get!(world, (player), Cash);
            set!(world, (Cash { player: player, amount: player_cash.amount + payout_cash }));

            // update player item
            let item = get!(world, (player, item_id), Item);
            let player_quantity = item.quantity;
            set!(
                world,
                (Item { player: player, item_id: item_id, quantity: player_quantity + payout_quantity })
            );

            // update player liquidity
            let player_liquidity = get!(world, (player, item_id), Liquidity);
            set!(
                world,
                (Liquidity {
                    player: player, item_id: item_id, shares: player_liquidity.shares - shares
                })
            );
        }
    }
}
