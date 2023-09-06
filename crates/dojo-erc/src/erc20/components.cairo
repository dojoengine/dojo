use integer::BoundedInt;
use starknet::ContractAddress;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct ERC20Allowance {
    #[key]
    token: ContractAddress,
    #[key]
    owner: ContractAddress,
    #[key]
    spender: ContractAddress,
    amount: u128,
}

trait ERC20AllowanceTrait {
    fn allowance(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress
    ) -> u128;
    fn approve(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        amount: u128
    );
    fn increase_allowance(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        added_value: u128
    );
    fn decrease_allowance(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        subtracted_value: u128
    );
    fn _spend_allowance(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        amount: u128
    );
}
impl ERC20AllowanceImpl of ERC20AllowanceTrait {
    fn allowance(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress
    ) -> u128 {
        get!(world, (token, owner, spender), ERC20Allowance).amount
    }

    fn approve(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        amount: u128
    ) {
        set!(world, ERC20Allowance { token, owner, spender, amount });
    }

    fn increase_allowance(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        added_value: u128
    ) {
        let mut allowance = get!(world, (token, owner, spender), ERC20Allowance);
        allowance.amount += added_value;
        set!(world, (allowance));
    }

    fn decrease_allowance(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        subtracted_value: u128
    ) {
        let mut allowance = get!(world, (token, owner, spender), ERC20Allowance);
        allowance.amount -= subtracted_value;
        set!(world, (allowance));
    }

    fn _spend_allowance(
        world: IWorldDispatcher,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        amount: u128
    ) {
        let current_allowance = get!(world, (token, owner, spender), ERC20Allowance).amount;
        if current_allowance != BoundedInt::max() {
            ERC20AllowanceTrait::approve(world, token, owner, spender, current_allowance - amount);
        }
    }
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct ERC20Balance {
    #[key]
    token: ContractAddress,
    #[key]
    account: ContractAddress,
    amount: u128,
}

trait ERC20BalanceTrait {
    fn balance_of(
        world: IWorldDispatcher, token: ContractAddress, account: ContractAddress
    ) -> u128;

    fn transfer_from(
        world: IWorldDispatcher,
        token: ContractAddress,
        sender: ContractAddress,
        recipient: ContractAddress,
        amount: u128
    );
    fn mint(
        world: IWorldDispatcher, token: ContractAddress, recipient: ContractAddress, amount: u128
    );
    fn burn(
        world: IWorldDispatcher, token: ContractAddress, account: ContractAddress, amount: u128
    );
}

impl ERC20BalanceImpl of ERC20BalanceTrait {
    fn balance_of(
        world: IWorldDispatcher, token: ContractAddress, account: ContractAddress
    ) -> u128 {
        get!(world, (token, account), ERC20Balance).amount
    }

    fn transfer_from(
        world: IWorldDispatcher,
        token: ContractAddress,
        sender: ContractAddress,
        recipient: ContractAddress,
        amount: u128
    ) {
        let mut sender_balance = get!(world, (token, sender), ERC20Balance);
        sender_balance.amount -= amount;
        set!(world, (sender_balance));

        let mut recipient_balance = get!(world, (token, recipient), ERC20Balance);
        recipient_balance.amount += amount;
        set!(world, (recipient_balance));
    }

    fn mint(
        world: IWorldDispatcher, token: ContractAddress, recipient: ContractAddress, amount: u128
    ) {
        // increase balance of recipient
        let mut balance = get!(world, (token, recipient), ERC20Balance);
        balance.amount += amount;
        set!(world, (balance));

        // increase token supply
        let mut supply = get!(world, token, ERC20Supply);
        supply.amount += amount;
        set!(world, (supply));
    }

    fn burn(
        world: IWorldDispatcher, token: ContractAddress, account: ContractAddress, amount: u128
    ) {
        // decrease balance of recipient
        let mut balance = get!(world, (token, account), ERC20Balance);
        balance.amount -= amount;
        set!(world, (balance));

        // decrease token supply
        let mut supply = get!(world, token, ERC20Supply);
        supply.amount -= amount;
        set!(world, (supply));
    }
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct ERC20Supply {
    #[key]
    token: ContractAddress,
    amount: u128
}
trait ERC20SupplyTrait {
    fn total_supply(world: IWorldDispatcher, token: ContractAddress,) -> u128;
}

impl ERC20SupplyImpl of ERC20SupplyTrait {
    fn total_supply(world: IWorldDispatcher, token: ContractAddress,) -> u128 {
        get!(world, (token), ERC20Supply).amount
    }
}
