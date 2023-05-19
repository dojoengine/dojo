#[system]
mod ERC20Approve {
    use traits::Into;
    use dojo_erc::erc20::components::Allowance;

    fn execute(token: felt252, owner: felt252, spender: felt252, amount: felt252) {
        commands::set_entity((token, (owner, spender)).into_partitioned(), (
            Allowance { amount }
        ))
    }
}

#[system]
mod ERC20TransferFrom {
    const UNLIMITED_ALLOWANCE: felt252 = 3618502788666131213697322783095070105623107215331596699973092056135872020480;

    use starknet::get_caller_address;
    use traits::Into;
    use zeroable::Zeroable;
    use dojo_erc::erc20::components::{Allowance, Balance};

    fn execute(token: felt252, spender: felt252, recipient: felt252, amount: felt252) {
        assert(spender.is_non_zero(), 'ERC20: transfer from 0');
        assert(recipient.is_non_zero(), 'ERC20: transfer to 0');

        let caller: felt252 = get_caller_address().into();
        if spender != caller {
            // decrease allowance if it's not owner doing the transfer
            let allowance = commands::<Allowance>::entity((token, (caller, spender)).into_partitioned());
            if !is_unlimited_allowance(allowance) {
                commands::set_entity((token, (caller, spender)).into_partitioned(), (
                    Allowance { amount: allowance.amount - amount }
                ));
            }
        }

        // decrease spender's balance
        let balance = commands::<Balance>::entity((token, (spender)).into_partitioned());
        commands::set_entity((token, (spender)).into_partitioned(), (
            Balance { amount: balance.amount - amount }
        ));

        // increase recipient's balance
        let balance = commands::<Balance>::entity((token, (recipient)).into_partitioned());
        commands::set_entity((token, (recipient)).into_partitioned(), (
            Balance { amount: balance.amount + amount }
        ));
    }

    fn is_unlimited_allowance(allowance: Allowance) -> bool {
        allowance.amount == UNLIMITED_ALLOWANCE
    }
}

#[system]
mod ERC20Mint {
    use traits::Into;
    use zeroable::Zeroable;
    use dojo_erc::erc20::components::{Balance, Supply};

    fn execute(token: felt252, recipient: felt252, amount: felt252) {
        assert(recipient.is_non_zero(), 'ERC20: mint to 0');

        // increase token supply
        let supply = commands::<Supply>::entity(token.into());
        commands::set_entity(token.into(), (
            Supply { amount: supply.amount + amount }
        ));

        // increase balance of recipient
        let balance = commands::<Balance>::entity((token, (recipient)).into_partitioned());
        commands::set_entity((token, (recipient)).into(), (
            Balance { amount: balance.amount + amount }
        ));
    }
}

#[system]
mod ERC20Burn {
    use traits::Into;
    use zeroable::Zeroable;        
    use dojo_erc::erc20::components::{Balance, Supply};

    fn execute(token: felt252, owner: felt252, amount: felt252) {
        assert(owner.is_non_zero(), 'ERC20: burn from 0');

        // decrease token supply
        let supply = commands::<Supply>::entity(token.into());
        commands::set_entity(token.into(), (
            Supply { amount: supply.amount - amount }
        ));

        // decrease balance of owner
        let balance = commands::<Balance>::entity((token, (owner)).into_partitioned());
        commands::set_entity((token, (owner)).into_partitioned(), (
            Balance { amount: balance.amount - amount }
        ));
    }
}