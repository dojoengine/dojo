#[system]
mod erc20_approve {
    use traits::Into;
    use starknet::ContractAddress;
    use dojo::world::Context;
    use dojo_erc::erc20::components::Allowance;

    use core::debug::PrintTrait;
    fn execute(
        ctx: Context,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        amount: felt252
    ) {
        set!(ctx.world, Allowance { token, owner, spender, amount })
    }
}

#[system]
mod erc20_transfer_from {
    const UNLIMITED_ALLOWANCE: felt252 =
        3618502788666131213697322783095070105623107215331596699973092056135872020480;

    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;
    use dojo::world::Context;
    use dojo_erc::erc20::components::{Allowance, Balance};

    use debug::PrintTrait;
    fn execute(
        ctx: Context,
        token: ContractAddress,
        sender: ContractAddress,
        recipient: ContractAddress,
        amount: felt252
    ) {
        assert(token == ctx.origin, 'ERC20: not authorized');
        let mut balance = get!(ctx.world, (token, sender), Balance);
        balance.amount -= amount;
        set!(ctx.world, (balance));

        // increase recipient's balance
        let mut balance = get!(ctx.world, (token, recipient), Balance);
        balance.amount += amount;
        set!(ctx.world, (balance));
    }

    fn is_unlimited_allowance(allowance: Allowance) -> bool {
        allowance.amount == UNLIMITED_ALLOWANCE
    }
}

#[system]
mod erc20_mint {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;
    use dojo::world::Context;
    use dojo_erc::erc20::components::{Balance, Supply};

    use debug::PrintTrait;

    fn execute(ctx: Context, token: ContractAddress, recipient: ContractAddress, amount: felt252) {
        assert(token == ctx.origin, 'ERC20: not authorized');
        assert(recipient.is_non_zero(), 'ERC20: mint to 0');
        // increase token supply
        let mut supply = get!(ctx.world, token, Supply);
        supply.amount += amount;
        set!(ctx.world, (supply));

        // increase balance of recipient
        let mut balance = get!(ctx.world, (token, recipient), Balance);
        balance.amount += amount;
        set!(ctx.world, (balance));
    }
}

#[system]
mod erc20_burn {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;
    use dojo::world::Context;
    use dojo_erc::erc20::components::{Balance, Supply};

    fn execute(ctx: Context, token: ContractAddress, owner: ContractAddress, amount: felt252) {
        assert(token == ctx.origin, 'ERC20: not authorized');
        assert(owner.is_non_zero(), 'ERC20: burn from 0');

        // decrease token supply
        let mut supply = get!(ctx.world, token, Supply);
        supply.amount -= amount;
        set!(ctx.world, (supply));

        // decrease balance of owner
        let mut balance = get!(ctx.world, (token, owner), Balance);
        balance.amount -= amount;
        set!(ctx.world, (balance));
    }
}
