#[system]
mod ERC20Approve {
    use traits::Into;
    use starknet::ContractAddress;
    use dojo::world::Context;
    use dojo_erc::erc20::components::Allowance;

    fn execute(
        ctx: Context,
        token: ContractAddress,
        owner: ContractAddress,
        spender: ContractAddress,
        amount: felt252
    ) {
        set !(ctx.world, Allowance { token, owner, spender, amount })
    }
}

#[system]
mod ERC20TransferFrom {
    const UNLIMITED_ALLOWANCE: felt252 =
        3618502788666131213697322783095070105623107215331596699973092056135872020480;

    use starknet::get_caller_address;
    use traits::Into;
    use zeroable::Zeroable;
    use dojo::world::Context;
    use dojo_erc::erc20::components::{Allowance, Balance};

    fn execute(
        ctx: Context, token: felt252, spender: felt252, recipient: felt252, amount: felt252
    ) {
        assert(spender.is_non_zero(), 'ERC20: transfer from 0');
        assert(recipient.is_non_zero(), 'ERC20: transfer to 0');

        let caller: felt252 = get_caller_address().into();
        if spender != caller {
            // decrease allowance if it's not owner doing the transfer
            let mut allowance = get !(ctx.world, (token, caller, spender), Allowance);
            if !is_unlimited_allowance(allowance) {
                allowance.amount -= amount;
                set !(ctx.world, (allowance));
            }
        }

        // decrease spender's balance
        let mut balance = get !(ctx.world, (token, spender), Balance);
        balance.amount -= amount;
        set !(ctx.world, (balance));

        // increase recipient's balance
        let mut balance = get !(ctx.world, (token, recipient), Balance);
        balance.amount += amount;
        set !(ctx.world, (balance));
    }

    fn is_unlimited_allowance(allowance: Allowance) -> bool {
        allowance.amount == UNLIMITED_ALLOWANCE
    }
}

#[system]
mod ERC20Mint {
    use traits::Into;
    use zeroable::Zeroable;
    use dojo::world::Context;
    use dojo_erc::erc20::components::{Balance, Supply};

    fn execute(ctx: Context, token: felt252, recipient: felt252, amount: felt252) {
        assert(recipient.is_non_zero(), 'ERC20: mint to 0');

        // increase token supply
        let mut supply = get !(ctx.world, token, Supply);
        supply.amount += amount;
        set !(ctx.world, (supply));

        // increase balance of recipient
        let mut balance = get !(ctx.world, (token, recipient), Balance);
        balance.amount -= amount;
        set !(ctx.world, (balance));
    }
}

#[system]
mod ERC20Burn {
    use traits::Into;
    use zeroable::Zeroable;
    use dojo::world::Context;
    use dojo_erc::erc20::components::{Balance, Supply};

    fn execute(ctx: Context, token: felt252, owner: felt252, amount: felt252) {
        assert(owner.is_non_zero(), 'ERC20: burn from 0');

        // decrease token supply
        let mut supply = get !(ctx.world, token, Supply);
        supply.amount -= amount;
        set !(ctx.world, (supply));

        // decrease balance of owner
        let mut balance = get !(ctx.world, (token, owner), Balance);
        balance.amount -= amount;
        set !(ctx.world, (balance));
    }
}
