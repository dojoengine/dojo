#[system]
mod ERC20Approve {
    use traits::Into;
    use array::ArrayTrait;
    use dojo_erc::erc20::components::Allowance;

    fn execute(token: felt252, owner: felt252, sender: felt252, amount: felt252) {
        // TODO: which query to use? token as key + (owner, spender) partition?
        //       or all three as keys?
        commands::set_entity((token, (owner, sender)).into(), (
            Allowance { amount }
        ))
    }
}

#[system]
mod ERC20TransferFrom {
    const UNLIMITED_ALLOWANCE: felt252 = 3618502788666131213697322783095070105623107215331596699973092056135872020480;

    use array::ArrayTrait;
    use starknet::get_caller_address;
    use traits::Into;
    use zeroable::Zeroable;
    use dojo_erc::erc20::components::Allowance;
    use dojo_erc::erc20::components::Balance;

    fn execute(token: felt252, sender: felt252, recipient: felt252, amount: felt252) {
        assert(sender.is_non_zero(), 'ERC20: transfer from 0');
        assert(recipient.is_non_zero(), 'ERC20: transfer to 0');

        let caller: felt252 = get_caller_address().into();
        if sender != caller {
            // decrease allowance if it's not owner doing the transfer
            let allowance = commands::<Allowance>::entity((token, (caller, sender)).into());
            if !is_unlimited_allowance(allowance) {
                commands::set_entity((token, (caller, sender)).into(), (
                    Allowance { amount: allowance.amount - amount }
                ));
            }
        }

        // decrease sender's balance
        let balance = commands::<Balance>::entity(sender.into());
        commands::set_entity(sender.into(), (
            Balance { amount: balance.amount - amount }
        ));

        // increase recipient's balance
        let balance = commands::<Balance>::entity(recipient.into());
        commands::set_entity(recipient.into(), (
            Balance { amount: balance.amount + amount }
        ));
    }

    fn is_unlimited_allowance(allowance: Allowance) -> bool {
        allowance.amount == UNLIMITED_ALLOWANCE
    }
}

#[system]
mod ERC20Mint {
    use array::ArrayTrait;
    use traits::Into;
    use dojo_erc::erc20::components::Balance;
    use dojo_erc::erc20::components::Supply;

    fn execute(token: felt252, recipient: felt252, amount: felt252) {
        // increase token supply
        let supply = commands::<Supply>::entity(token.into());
        commands::set_entity(token.into(), (
            Supply { amount: supply.amount + amount }
        ));

        // increase balance of recipient
        let balance = commands::<Balance>::entity(recipient.into());
        commands::set_entity(recipient.into(), (
            Balance { amount: balance.amount + amount }
        ));
    }
}
