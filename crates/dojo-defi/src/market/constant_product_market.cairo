use traits::{Into, TryInto};
use option::OptionTrait;
use starknet::ContractAddress;

use dojo_defi::market::components::Market;

use cubit::f128::types::fixed::{Fixed, FixedTrait};

const SCALING_FACTOR: u128 = 10000;

#[generate_trait]
impl MarketImpl of MarketTrait {
    fn buy(self: @Market, quantity: u128) -> u128 {
        assert(quantity < *self.item_quantity, 'not enough liquidity');
        let (quantity, available, cash) = normalize(quantity, self);
        let k = cash * available;
        let cost = (k / (available - quantity)) - cash;
        cost
    }

    fn sell(self: @Market, quantity: u128) -> u128 {
        let (quantity, available, cash) = normalize(quantity, self);
        let k = cash * available;
        let payout = cash - (k / (available + quantity));
        payout
    }

    // Get normalized reserve cash amount and item quantity
    fn get_reserves(self: @Market) -> (u128, u128) {
        let reserve_quantity: u128 = (*self.item_quantity).into() * SCALING_FACTOR;
        (*self.cash_amount, reserve_quantity)
    }

    // Get the liquidity of the market
    // Use cubit fixed point math library to compute the square root of the product of the reserves
    fn liquidity(self: @Market) -> Fixed {
        // Get normalized reserve cash amount and item quantity
        let (reserve_amount, reserve_quantity) = self.get_reserves();

        // Convert reserve amount to fixed point
        let reserve_amount = FixedTrait::new_unscaled(reserve_amount, false);
        let reserve_quantity = FixedTrait::new_unscaled(reserve_quantity, false);

        // L = sqrt(X * Y)
        (reserve_amount * reserve_quantity).sqrt()
    }

    // Check if the market has liquidity
    fn has_liquidity(self: @Market) -> bool {
        *self.cash_amount > 0 || *self.item_quantity > 0
    }

    // Given some amount of cash, return the equivalent/optimal quantity of items
    // based on the reserves in the market
    fn quote_quantity(self: @Market, amount: u128) -> u128 {
        assert(amount > 0, 'insufficient amount');
        assert(self.has_liquidity(), 'insufficient liquidity');

        // Get normalized reserve cash amount and item quantity
        let (reserve_amount, reserve_quantity) = self.get_reserves();

        // Convert amount to fixed point
        let amount = FixedTrait::new_unscaled(amount, false);

        // Convert reserve amount and quantity to fixed point
        let reserve_amount = FixedTrait::new_unscaled(reserve_amount, false);
        let reserve_quantity = FixedTrait::new_unscaled(reserve_quantity, false);

        // dy = Y * dx / X
        let quantity_optimal = (reserve_quantity * amount) / reserve_amount;

        // Convert from fixed point to u128
        let res: u128 = quantity_optimal.try_into().unwrap();
        res
    }

    // Given some quantity of items, return the equivalent/optimal amount of cash
    // based on the reserves in the market
    fn quote_amount(self: @Market, quantity: u128) -> u128 {
        assert(quantity > 0, 'insufficient quantity');
        assert(self.has_liquidity(), 'insufficient liquidity');

        // Get normalized reserve cash amount and item quantity
        let (reserve_amount, reserve_quantity) = self.get_reserves();

        // Convert reserve amount and quantity to fixed point
        let reserve_amount = FixedTrait::new_unscaled(reserve_amount, false);
        let reserve_quantity = FixedTrait::new_unscaled(reserve_quantity, false);

        // Normalize quantity
        let quantity: u128 = quantity.into() * SCALING_FACTOR;

        // Convert quantity to fixed point
        let quantity = FixedTrait::new_unscaled(quantity, false);

        // dx = X * dy / Y
        let amount_optimal = (reserve_amount * quantity) / reserve_quantity;

        // Convert from fixed point to u128
        amount_optimal.try_into().unwrap()
    }

    // Inner function to add liquidity to the market, computes the optimal amount and quantity
    //
    // Arguments:
    //
    // amount: The amount of cash to add to the market
    // quantity: The quantity of items to add to the market
    //
    // Returns:
    //
    // (amount, quantity): The amount of cash and quantity of items added to the market
    fn add_liquidity_inner(self: @Market, amount: u128, quantity: u128) -> (u128, u128) {
        // If there is no liquidity, then the amount and quantity are the optimal
        if !self.has_liquidity() {
            // Ensure that the amount and quantity are greater than zero
            assert(amount > 0, 'insufficient amount');
            assert(quantity > 0, 'insufficient quantity');
            (amount, quantity)
        } else {
            // Given the amount, get optimal quantity to add to the market
            let quantity_optimal = self.quote_quantity(amount);
            if quantity_optimal <= quantity {
                // Add the given amount and optimal quantity to the market
                (amount, quantity_optimal)
            } else {
                let amount_optimal = self.quote_amount(quantity);
                // Ensure that the optimal amount is less than or equal to the given amount
                assert(amount_optimal <= amount, 'insufficient amount');
                (amount_optimal, quantity)
            }
        }
    }

    // Add liquidity to the market, mints shares for the given amount of liquidity provided
    //
    // Arguments:
    //
    // amount: The amount of cash to add to the market
    // quantity: The quantity of items to add to the market
    //
    // Returns:
    //
    // (amount, quantity, shares): The amount of cash and quantity of items added to the market and the shares minted
    fn add_liquidity(self: @Market, amount: u128, quantity: u128) -> (u128, u128, Fixed) {
        // Compute the amount and quantity to add to the market
        let (amount, quantity) = self.add_liquidity_inner(amount, quantity);
        // Mint shares for the given amount of liquidity provided
        let shares = self.mint_shares(amount, quantity);
        (amount, quantity, shares)
    }

    // Mint shares for the given amount of liquidity provided
    fn mint_shares(self: @Market, amount: u128, quantity: u128) -> Fixed {
        // If there is no liquidity, then mint total shares
        if !self.has_liquidity() {
            let quantity: u128 = quantity.into() * SCALING_FACTOR;
            (FixedTrait::new_unscaled(amount, false) * FixedTrait::new_unscaled(quantity, false))
                .sqrt()
        } else {
            // Convert amount to fixed point
            let amount = FixedTrait::new_unscaled(amount, false);

            // Get normalized reserve cash amount and item quantity
            let (reserve_amount, _) = self.get_reserves();

            // Convert reserve amount to fixed point
            let reserve_amount = FixedTrait::new_unscaled(reserve_amount, false);

            // Get total liquidity
            let liquidity = self.liquidity();

            // Compute the amount of shares to mint
            // S = dx * L/X = dy * L/Y
            (amount * liquidity) / reserve_amount
        }
    }

    // Remove liquidity from the market, return the corresponding amount and quantity payout
    //
    // Arguments:
    //
    // shares: The amount of liquidity shares to remove from the market
    //
    // Returns:
    //
    // (amount, quantity): The amount of cash and quantity of items removed from the market
    fn remove_liquidity(self: @Market, shares: Fixed) -> (u128, u128) {
        // Ensure that the market has liquidity
        let liquidity = self.liquidity();
        assert(shares <= liquidity, 'insufficient liquidity');

        // Get normalized reserve cash amount and item quantity
        let (reserve_amount, reserve_quantity) = self.get_reserves();

        // Convert reserve amount and quantity to fixed point
        let reserve_amount = FixedTrait::new_unscaled(reserve_amount, false);
        let reserve_quantity = FixedTrait::new_unscaled(reserve_quantity, false);

        // Compute the amount and quantity to remove from the market
        // dx = S * X / L
        let amount = (shares * reserve_amount) / liquidity;
        // dy = S * Y / L
        let quantity = (shares * reserve_quantity) / liquidity;

        // Convert amount and quantity both from fixed point to u128 and unscaled u128, respectively
        (amount.try_into().unwrap(), quantity.try_into().unwrap() / SCALING_FACTOR)
    }
}

fn normalize(quantity: u128, market: @Market) -> (u128, u128, u128) {
    let quantity: u128 = quantity.into() * SCALING_FACTOR;
    let available: u128 = (*market.item_quantity).into() * SCALING_FACTOR;
    (quantity, available, *market.cash_amount)
}
