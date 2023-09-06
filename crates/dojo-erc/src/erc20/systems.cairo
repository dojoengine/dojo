use array::{ArrayTrait, SpanTrait};
use clone::Clone;
use starknet::ContractAddress;
use traits::Into;


use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_erc::erc20::erc20::ERC20::{
    IERC20EventsDispatcher, IERC20EventsDispatcherTrait, Transfer, Approval, Event
};

use ERC20Approve::ERC20ApproveParams;
use ERC20DecreaseAllowance::ERC20DecreaseAllowanceParams;
use ERC20IncreaseAllowance::ERC20IncreaseAllowanceParams;
use ERC20Mint::ERC20MintParams;
use ERC20TransferFrom::ERC20TransferFromParams;

fn emit_transfer(
    world: IWorldDispatcher,
    token: ContractAddress,
    from: ContractAddress,
    to: ContractAddress,
    value: u128,
) {
    let event = Transfer { from, to, value: value.into() };
    IERC20EventsDispatcher { contract_address: token }.on_transfer(event.clone());
    emit!(world, event);
}

fn emit_approval(
    world: IWorldDispatcher,
    token: ContractAddress,
    owner: ContractAddress,
    spender: ContractAddress,
    amount: u128,
) {
    let event = Approval { owner, spender, value: amount.into() };
    IERC20EventsDispatcher { contract_address: token }.on_approval(event.clone());
    emit!(world, event);
}

#[system]
mod ERC20Approve {
    use starknet::ContractAddress;
    use zeroable::Zeroable;

    use dojo_erc::erc20::components::ERC20AllowanceTrait;
    use dojo::world::Context;

    #[derive(Drop, Serde)]
    struct ERC20ApproveParams {
        token: ContractAddress,
        caller: ContractAddress,
        spender: ContractAddress,
        amount: u128
    }

    fn execute(ctx: Context, params: ERC20ApproveParams) {
        let ERC20ApproveParams{token, caller, spender, amount } = params;

        assert(!caller.is_zero(), 'ERC20: approve from 0');
        assert(!spender.is_zero(), 'ERC20: approve to 0');
        ERC20AllowanceTrait::approve(ctx.world, token, caller, spender, amount);

        super::emit_approval(ctx.world, token, caller, spender, amount);
    }
}

#[system]
mod ERC20IncreaseAllowance {
    use starknet::ContractAddress;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc20::components::ERC20AllowanceTrait;

    #[derive(Drop, Serde)]
    struct ERC20IncreaseAllowanceParams {
        token: ContractAddress,
        caller: ContractAddress,
        spender: ContractAddress,
        added_value: u128
    }

    fn execute(ctx: Context, params: ERC20IncreaseAllowanceParams) {
        let ERC20IncreaseAllowanceParams{token, caller, spender, added_value } = params;
        assert(!spender.is_zero(), 'ERC20: approve to 0');
        assert(!caller.is_zero(), 'ERC20: approve from 0');
        ERC20AllowanceTrait::decrease_allowance(ctx.world, token, caller, spender, added_value);
    }
}

#[system]
mod ERC20DecreaseAllowance {
    use starknet::ContractAddress;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc20::components::ERC20AllowanceTrait;

    #[derive(Drop, Serde)]
    struct ERC20DecreaseAllowanceParams {
        token: ContractAddress,
        caller: ContractAddress,
        spender: ContractAddress,
        subtracted_value: u128
    }

    fn execute(ctx: Context, params: ERC20DecreaseAllowanceParams) {
        let ERC20DecreaseAllowanceParams{token, caller, spender, subtracted_value } = params;
        assert(!spender.is_zero(), 'ERC20: approve to 0');
        assert(!caller.is_zero(), 'ERC20: approve from 0');
        ERC20AllowanceTrait::decrease_allowance(
            ctx.world, token, caller, spender, subtracted_value
        );
    }
}


#[system]
mod ERC20TransferFrom {
    use starknet::ContractAddress;
    use zeroable::Zeroable;

    use dojo_erc::erc20::components::{ERC20AllowanceTrait, ERC20BalanceTrait};
    use dojo::world::Context;

    #[derive(Drop, Serde)]
    struct ERC20TransferFromParams {
        token: ContractAddress,
        sender: ContractAddress,
        caller: ContractAddress,
        recipient: ContractAddress,
        amount: u128,
    }


    fn execute(ctx: Context, params: ERC20TransferFromParams) {
        let ERC20TransferFromParams{token, sender, caller, recipient, amount } = params;
        assert(token == ctx.origin, 'ERC20: not authorized');
        assert(!sender.is_zero(), 'ERC20: transfer from 0');
        assert(!recipient.is_zero(), 'ERC20: transfer to 0');
        if sender != caller {
            ERC20AllowanceTrait::_spend_allowance(ctx.world, token, sender, caller, amount);
        }
        ERC20BalanceTrait::transfer_from(ctx.world, token, sender, recipient, amount);

        super::emit_transfer(ctx.world, token, sender, recipient, amount);
    }
}

#[system]
mod ERC20Mint {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc20::components::ERC20BalanceTrait;

    #[derive(Drop, Serde)]
    struct ERC20MintParams {
        token: ContractAddress,
        recipient: ContractAddress,
        amount: u128,
    }

    fn execute(ctx: Context, params: ERC20MintParams) {
        let ERC20MintParams{token, recipient, amount } = params;
        assert(token == ctx.origin, 'ERC20: not authorized');
        assert(!recipient.is_zero(), 'ERC20: mint to 0');
        ERC20BalanceTrait::mint(ctx.world, token, recipient, amount);
    }
}

#[system]
mod ERC20Burn {
    use starknet::ContractAddress;
    use traits::Into;
    use zeroable::Zeroable;

    use dojo::world::Context;
    use dojo_erc::erc20::components::ERC20BalanceTrait;

    #[derive(Drop, Serde)]
    struct ERC20BurnParams {
        token: ContractAddress,
        account: ContractAddress,
        amount: u128,
    }

    fn execute(ctx: Context, params: ERC20BurnParams) {
        let ERC20BurnParams{token, account, amount } = params;
        assert(token == ctx.origin, 'ERC20: not authorized');
        assert(!account.is_zero(), 'ERC20: burn from 0');
        ERC20BalanceTrait::burn(ctx.world, token, account, amount);
    }
}
